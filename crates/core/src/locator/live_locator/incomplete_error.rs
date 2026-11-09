//! The one place a locator that cannot prove its result becomes an error.
//!
//! Observation stats and the operation deadline determine the cause. The
//! suggestion names the lever that helps: a larger `--timeout-ms` only when the
//! deadline ran out, a narrower query when more time cannot change the answer.

use super::{LocatorResolution, LocatorStats, ObservedNode, ObservedTree};
use crate::{AdapterError, ErrorCode};
use serde_json::{Value, json};

const DEADLINE_SUGGESTION: &str =
    "Retry with a larger --timeout-ms, or narrow the query with --role or --name.";

const NARROW_THE_QUERY_SUGGESTION: &str = "More time will not help. Narrow the query with --role or --name, or run snapshot, which returns the readable part of the tree with complete:false.";

#[derive(Clone, Copy, PartialEq, Eq)]
enum IncompleteCause {
    Deadline,
    TraversalLimit,
    UnreadableElements,
}

impl IncompleteCause {
    fn of(stats: &LocatorStats) -> Self {
        let health = &stats.reads.health;
        if health.deadline_exhausted > 0 {
            Self::Deadline
        } else if health.native_read_failures == 0 && has_deterministic_limit(stats) {
            Self::TraversalLimit
        } else {
            Self::UnreadableElements
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Deadline => "deadline",
            Self::TraversalLimit => "traversal_limit",
            Self::UnreadableElements => "unreadable_elements",
        }
    }

    fn suggestion(self) -> &'static str {
        match self {
            Self::Deadline => DEADLINE_SUGGESTION,
            Self::TraversalLimit | Self::UnreadableElements => NARROW_THE_QUERY_SUGGESTION,
        }
    }

    fn message(self, stats: &LocatorStats) -> String {
        match self {
            Self::Deadline => {
                "The locator search ran out of time before it could prove its result".to_owned()
            }
            Self::TraversalLimit => format!(
                "The locator search hit a traversal limit ({}) and cannot prove its result",
                limits_hit(stats).join(", ")
            ),
            Self::UnreadableElements => match failed_reads(stats) {
                0 => "Part of the tree could not be read, so the locator search cannot prove its result".to_owned(),
                1 => "1 accessibility read failed, so the locator search cannot prove its result".to_owned(),
                count => format!(
                    "{count} accessibility reads failed, so the locator search cannot prove its result"
                ),
            },
        }
    }
}

fn failed_reads(stats: &LocatorStats) -> u64 {
    stats.reads.health.native_read_failures + stats.reads.health.cannot_complete
}

fn limits_hit(stats: &LocatorStats) -> Vec<&'static str> {
    let limits = &stats.traversal.limits;
    [
        ("node", limits.node_hits),
        ("edge", limits.edge_hits),
        ("child", limits.child_hits),
        ("text", limits.text_hits),
        ("depth", limits.depth_hits),
    ]
    .into_iter()
    .filter(|(_, hits)| *hits > 0)
    .map(|(name, _)| name)
    .collect()
}

pub(super) fn has_deterministic_limit(stats: &LocatorStats) -> bool {
    let limits = &stats.traversal.limits;
    limits.node_hits > 0
        || limits.edge_hits > 0
        || limits.child_hits > 0
        || limits.text_hits > 0
        || limits.depth_hits > 0
}

pub(super) enum IncompleteLocator<'a> {
    Resolution(&'a LocatorResolution),
    Search(&'a LocatorResolution),
    Deadline(crate::Deadline, &'a LocatorStats, Option<Value>),
    SelectedQuery(&'a LocatorResolution),
    SelectedEvidence(
        &'a ObservedTree,
        Option<&'a ObservedNode>,
        &'a crate::LocatorQuery,
        crate::BoundsSpace,
    ),
}

impl IncompleteLocator<'_> {
    pub(super) fn error(self) -> AdapterError {
        let (stats, kind, mut details) = match &self {
            Self::Resolution(resolution) | Self::Search(resolution) => {
                let budget =
                    matches!(self, Self::Search(_)) && has_deterministic_limit(&resolution.stats);
                let mut details = json!({
                    "observed_matches": resolution.meta.total_matches,
                    "roles_present": resolution.meta.roles_present,
                });
                if budget {
                    details["limits"] = json!(resolution.stats.traversal.limits);
                }
                (
                    &resolution.stats,
                    if budget {
                        "locator_budget_limit"
                    } else {
                        "locator_incomplete"
                    },
                    details,
                )
            }
            Self::Deadline(deadline, stats, last_incomplete) => (
                *stats,
                "locator_transient_incomplete",
                json!({
                    "timeout_ms": deadline.timeout_ms(),
                    "observation_attempts": stats.reads.counts.observation_attempts,
                    "last_incomplete": last_incomplete,
                }),
            ),
            Self::SelectedQuery(resolution) => (
                &resolution.stats,
                if has_deterministic_limit(&resolution.stats) {
                    "locator_selected_query_budget_limit"
                } else {
                    "locator_selected_query_incomplete"
                },
                json!({
                    "selection_complete": resolution.meta.selection_complete,
                    "observed_matches": resolution.meta.total_matches,
                }),
            ),
            Self::SelectedEvidence(tree, node, query, space) => (
                &tree.stats,
                "locator_selected_evidence_incomplete",
                json!({
                    "structurally_complete": tree.structurally_complete,
                    "root_count": tree.roots.len(),
                    "required": {
                        "name": query.identity.name.is_some(),
                        "description": query.identity.description.is_some(),
                        "value": query.identity.value.is_some(),
                        "identifiers": query.identity.native_id.is_some(),
                        "states": true,
                        "bounds": true,
                        "actions": true,
                    },
                    "unknown": node.map(|node| json!({
                        "role": node.evidence.role.is_unknown(),
                        "name": node.evidence.name.is_unknown(),
                        "description": node.evidence.description.is_unknown(),
                        "value": node.evidence.value.is_unknown(),
                        "identifiers": !node.evidence.identifiers.is_complete(),
                        "states": node.evidence.states.is_unknown(),
                        "bounds": !super::hydrate::geometry_complete(&node.evidence.ref_evidence, *space),
                        "actions": node.evidence.ref_evidence.available_actions.is_unknown(),
                    })),
                }),
            ),
        };
        let cause = if matches!(self, Self::Deadline(..)) {
            IncompleteCause::Deadline
        } else {
            IncompleteCause::of(stats)
        };
        details["kind"] = json!(kind);
        details["cause"] = json!(cause.as_str());
        details["retryable"] = json!(cause == IncompleteCause::Deadline);
        details["query_stats"] = json!(stats);
        AdapterError::new(ErrorCode::Timeout, cause.message(stats))
            .with_suggestion(cause.suggestion())
            .with_details(details)
            .with_disposition(crate::DeliverySemantics::not_delivered())
    }
}

/// Builds the `TIMEOUT` for a resolution that could not prove its result.
pub fn incomplete_locator_error(resolution: &LocatorResolution) -> AdapterError {
    IncompleteLocator::Resolution(resolution).error()
}

#[cfg(test)]
#[path = "incomplete_error_tests.rs"]
mod tests;
