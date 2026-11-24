use super::baseline::SnapshotBaseline;
use super::delta::{BaselineRef, SnapshotDelta, diff_baselines};
use super::since_status::SinceStatus;
use crate::{AccessibilityNode, snapshot::SnapshotResult};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub(crate) enum BaselineLookup {
    Found(SnapshotBaseline),
    Expired,
    Unknown,
}

#[derive(Debug, Clone)]
pub(crate) struct SinceEvaluation {
    pub(crate) baseline_id: String,
    pub(crate) status: SinceStatus,
    pub(crate) delta: Option<SnapshotDelta>,
}

pub(crate) fn evaluate_since(
    baseline_id: &str,
    lookup: BaselineLookup,
    new_baseline: &SnapshotBaseline,
    new_snapshot_id: &str,
    new_raw_tree: &AccessibilityNode,
) -> SinceEvaluation {
    let old = match lookup {
        BaselineLookup::Expired => {
            return fallback(baseline_id, SinceStatus::Expired);
        }
        BaselineLookup::Unknown => {
            return fallback(baseline_id, SinceStatus::UnknownBaseline);
        }
        BaselineLookup::Found(baseline) => baseline,
    };

    if !old.complete || !new_baseline.complete {
        return fallback(baseline_id, SinceStatus::Incomplete);
    }
    if old.owner != new_baseline.owner {
        return fallback(baseline_id, SinceStatus::WindowMismatch);
    }
    if old.scope != new_baseline.scope {
        return fallback(baseline_id, SinceStatus::ScopeMismatch);
    }
    if old.tree_hash == new_baseline.tree_hash {
        return SinceEvaluation {
            baseline_id: baseline_id.to_owned(),
            status: SinceStatus::Unchanged,
            delta: None,
        };
    }

    let delta = diff_baselines(
        BaselineRef {
            snapshot_id: baseline_id,
            baseline: &old,
        },
        BaselineRef {
            snapshot_id: new_snapshot_id,
            baseline: new_baseline,
        },
        new_raw_tree,
    );
    if delta.is_empty() {
        return SinceEvaluation {
            baseline_id: baseline_id.to_owned(),
            status: SinceStatus::Unchanged,
            delta: None,
        };
    }

    SinceEvaluation {
        baseline_id: baseline_id.to_owned(),
        status: SinceStatus::Delta,
        delta: Some(delta),
    }
}

fn fallback(baseline_id: &str, status: SinceStatus) -> SinceEvaluation {
    SinceEvaluation {
        baseline_id: baseline_id.to_owned(),
        status,
        delta: None,
    }
}

pub(crate) fn format_since_snapshot_fields(
    result: &SnapshotResult,
    since: &SinceEvaluation,
    elapsed_ms: Option<u128>,
    matched_selector: Option<&str>,
) -> Result<Value, crate::AppError> {
    let ref_count = result.refmap.len();
    let mut body = json!({
        "app": result.owner.app_name(),
        "ref_count": ref_count,
        "snapshot_id": result.snapshot_id,
        "complete": result.complete,
        "since": since.baseline_id,
        "since_status": since.status.as_str(),
    });
    if let Some(window) = result.owner.window() {
        body["window"] = json!({ "id": window.id, "title": window.title });
    }
    if !result.complete {
        body["truncated"] = json!(true);
        body["nodes_observed"] = json!(result.nodes_observed);
    }
    if result.identity_unproven.is_some() {
        body["identity_unproven"] = json!(true);
    }
    if let Some(ms) = elapsed_ms {
        body["elapsed_ms"] = json!(ms);
    }
    if let Some(selector) = matched_selector {
        body["matched_selector"] = json!(selector);
    }
    match since.status {
        SinceStatus::Unchanged => {
            body["unchanged"] = json!(true);
        }
        SinceStatus::Delta => {
            body["unchanged"] = json!(false);
            body["delta"] = serde_json::to_value(&since.delta)?;
        }
        SinceStatus::UnknownBaseline
        | SinceStatus::Expired
        | SinceStatus::WindowMismatch
        | SinceStatus::ScopeMismatch
        | SinceStatus::Incomplete => {
            body["tree"] = serde_json::to_value(&result.tree)?;
        }
    }
    Ok(body)
}

pub(crate) fn persist_wait_snapshot(
    mut result: SnapshotResult,
    context: &crate::context::CommandContext,
    opts: &crate::adapter::TreeOptions,
    since: Option<&str>,
) -> Result<(SnapshotResult, Option<SinceEvaluation>), crate::AppError> {
    let new_raw_tree = result.tree.clone();
    let new_baseline = super::baseline::SnapshotBaseline::from_snapshot_result(&result, opts, None);
    super::baseline::redact_sensitive_tree_values(&mut result);

    let store = crate::refs::refs_store::RefStore::for_session(context.session_id())?;
    let lookup = match since {
        Some(baseline_id) => Some(store.load_baseline(baseline_id)?),
        None => None,
    };
    let snapshot_id = result.save_refmap_with_baseline(&store, Some(&new_baseline))?;
    crate::trace::trace_artifacts::copy_refmap_if_full(
        context,
        &store,
        &snapshot_id,
        &result.refmap,
    )?;
    result.bind_snapshot_id(snapshot_id.clone());
    let evaluation = match (since, lookup) {
        (Some(baseline_id), Some(lookup)) => Some(evaluate_since(
            baseline_id,
            lookup,
            &new_baseline,
            &snapshot_id,
            &new_raw_tree,
        )),
        _ => None,
    };
    super::emit_snapshot_saved_with_since(context, &result, evaluation.as_ref())?;
    Ok((result, evaluation))
}

#[cfg(test)]
#[path = "since_tests.rs"]
mod tests;
