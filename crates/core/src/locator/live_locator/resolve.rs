use super::incomplete_error::{IncompleteLocator, has_deterministic_limit};
use super::{
    LocatorResolution, LocatorResolveRequest, LocatorStats, ObservationRequest, ObservationRoot,
    ObservedObservation, validate_query, validate_request,
};
use crate::{AppError, adapter::PlatformAdapter, locator::LocatorQuery};
use serde_json::json;
use std::time::{Duration, Instant};

pub fn resolve_query(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    root: ObservationRoot<'_>,
    request: &LocatorResolveRequest,
) -> Result<LocatorResolution, AppError> {
    resolve_query_from(adapter, query, request, |deadline| {
        let observation_request =
            ObservationRequest::locator_for_root(query, request, root, deadline).validate()?;
        crate::tree::renderer_accessibility::observe_tree(adapter, root, &observation_request, None)
            .map(ObservedObservation::whole)
    })
}

/// Resolves a query over trees the caller observes. A caller that already holds
/// the read its root resolution took hands that same tree back here, so the refs
/// it publishes are proven by one observation rather than by a second one.
pub(crate) fn resolve_query_from(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    request: &LocatorResolveRequest,
    mut observe: impl FnMut(crate::Deadline) -> Result<ObservedObservation, AppError>,
) -> Result<LocatorResolution, AppError> {
    validate_query(query)?;
    validate_request(request)?;
    let started = Instant::now();
    let deadline = request.deadline;
    let mut aggregate = LocatorStats::default();
    let mut last_incomplete = None;
    let mut selection_retry_used = false;
    loop {
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return Err(
                IncompleteLocator::Deadline(deadline, &aggregate, last_incomplete)
                    .error()
                    .into(),
            );
        }
        match observe(observation_deadline(request)).and_then(|observed| {
            resolve_observed_attempt(adapter, query, observed, request, &mut aggregate)
        }) {
            Ok(mut resolution) => {
                resolution.stats.reads.counts.observation_attempts =
                    resolution.stats.reads.counts.observation_attempts.max(1);
                if !resolution.meta.selection_complete && has_deterministic_limit(&resolution.stats)
                {
                    return Err(IncompleteLocator::Search(&resolution).error().into());
                }
                if !resolution.meta.selection_complete
                    && has_transient_incompleteness(&resolution.stats)
                {
                    last_incomplete = Some(incomplete_evidence(&resolution));
                    aggregate.merge_attempt(&resolution.stats);
                    if deadline.is_expired() {
                        return Err(IncompleteLocator::Deadline(
                            deadline,
                            &aggregate,
                            last_incomplete,
                        )
                        .error()
                        .into());
                    }
                    let remaining = deadline.remaining();
                    crate::wait::deadline::sleep(remaining.min(Duration::from_millis(10)));
                    continue;
                }
                if !resolution.meta.selection_complete && requires_authoritative_result(request) {
                    return Err(IncompleteLocator::Search(&resolution).error().into());
                }
                aggregate.merge_attempt(&resolution.stats);
                aggregate.elapsed_us =
                    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
                resolution.stats = aggregate;
                return Ok(resolution);
            }
            Err(AppError::Adapter(error))
                if request.materialization == super::LocatorMaterialization::SelectedMatches
                    && super::hydrate::retryable_error(&error)
                    && !selection_retry_used
                    && !deadline.is_expired() =>
            {
                selection_retry_used = true;
                last_incomplete = Some(json!({
                    "phase": "hydration",
                    "code": error.code.as_str(),
                    "details": error.details,
                }));
                let remaining = deadline.remaining();
                crate::wait::deadline::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(error) => return Err(error),
        }
    }
}

fn requires_authoritative_result(request: &LocatorResolveRequest) -> bool {
    request.materialization == super::LocatorMaterialization::SelectedMatches
        || !matches!(
            request.selection,
            super::LocatorSelection::All { .. } | super::LocatorSelection::Count
        )
}

fn has_transient_incompleteness(stats: &LocatorStats) -> bool {
    stats.traversal.limits.child_count_changes > 0
        || stats.reads.health.cannot_complete > 0
        || stats.reads.health.deadline_exhausted > 0
}

fn incomplete_evidence(resolution: &LocatorResolution) -> serde_json::Value {
    json!({
        "observed_matches": resolution.meta.total_matches,
        "child_count_changes": resolution.stats.traversal.limits.child_count_changes,
        "cannot_complete": resolution.stats.reads.health.cannot_complete,
        "deadline_exhausted": resolution.stats.reads.health.deadline_exhausted,
    })
}

fn resolve_observed_attempt(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    observed: ObservedObservation,
    request: &LocatorResolveRequest,
    aggregate: &mut LocatorStats,
) -> Result<LocatorResolution, AppError> {
    let ObservedObservation { mut tree, scope } = observed;
    tree.stats.reads.counts.observation_attempts =
        tree.stats.reads.counts.observation_attempts.max(1);
    let evaluation_request = LocatorResolveRequest {
        materialization: match request.materialization {
            super::LocatorMaterialization::SelectedMatches => super::LocatorMaterialization::None,
            other => other,
        },
        ..*request
    };
    let (mut resolution, tree) = super::evaluate::evaluate_locator_tree_in(
        tree,
        scope.as_ref(),
        query,
        &evaluation_request,
    )?;
    if request.materialization == super::LocatorMaterialization::SelectedMatches {
        if let Err(error) =
            super::hydrate::selected_matches(adapter, query, request, &mut resolution, &tree)
        {
            aggregate.merge_attempt(&resolution.stats);
            return Err(error);
        }
    }
    Ok(resolution)
}

fn observation_deadline(request: &LocatorResolveRequest) -> crate::Deadline {
    if request.materialization != super::LocatorMaterialization::SelectedMatches {
        return request.deadline;
    }
    let remaining = request.deadline.remaining();
    let reserve = (remaining / 2).min(Duration::from_secs(1));
    request.deadline.capped(remaining.saturating_sub(reserve))
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn selected_ref_hydration_has_budget_without_extending_the_operation() {
        let request = LocatorResolveRequest {
            selection: super::super::LocatorSelection::First,
            deadline: crate::Deadline::after(60_000).unwrap(),
            max_raw_depth: 50,
            surface: None,
            materialization: super::super::LocatorMaterialization::SelectedMatches,
        };
        let observation = observation_deadline(&request);
        assert!(observation.was_capped());
        assert!(!request.deadline.was_capped());
        assert_eq!(observation.timeout_ms(), request.deadline.timeout_ms());
        let count_request = LocatorResolveRequest {
            materialization: super::super::LocatorMaterialization::None,
            ..request
        };
        assert_eq!(observation_deadline(&count_request), request.deadline);
    }
}
