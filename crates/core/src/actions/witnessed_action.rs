//! The effect witness around an action whose effect lands outside the
//! target's own state: a click, a select. The adapter reads a witness before
//! the action; the dispatch result chooses one of two explicit steps; a strong
//! change upgrades a delivery to verified, and the verdict and the changes are
//! reported. A delivered result never becomes undelivered, an unclear answer
//! with a strong change becomes a verified success, and the witness never
//! makes an error of its own.

use crate::actions::{
    effect_witness::{EffectWitness, verdict},
    observed_effect::ObservedEffect,
    witness_change::WitnessChange,
};
use crate::{
    ActionRequest, ActionResult, AdapterError, DeliverySemantics, ErrorCode, InteractionLease,
    NativeHandle, PlatformAdapter,
};

/// What the witness does once the action was dispatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WitnessStep {
    /// Nothing was sent, or the adapter already proved the effect: no read.
    Leave,
    /// Sent without proof, or the platform's answer was unclear: one read, no
    /// wait. An effect that lands later is for the caller to wait for.
    Read,
}

pub(crate) fn witness_step(dispatch: &Result<ActionResult, AdapterError>) -> WitnessStep {
    match dispatch {
        Ok(result) if result.disposition() == DeliverySemantics::delivered_unverified() => {
            WitnessStep::Read
        }
        Err(error) if error.disposition == DeliverySemantics::uncertain() => WitnessStep::Read,
        Ok(_) | Err(_) => WitnessStep::Leave,
    }
}

pub(crate) fn execute_witnessed_action(
    adapter: &dyn PlatformAdapter,
    handle: &NativeHandle,
    request: ActionRequest,
    lease: &InteractionLease,
) -> Result<ActionResult, AdapterError> {
    let deadline = lease.deadline();
    let action_name = request.action.name();
    let before = adapter.read_effect_witness(handle, deadline).ok();
    let dispatch = adapter
        .execute_action(handle, request, lease)
        .map_err(crate::actions::post_action::settle_disposition);
    let Some(before) = before else {
        return dispatch;
    };
    let changes = match witness_step(&dispatch) {
        WitnessStep::Leave => None,
        WitnessStep::Read => after_changes(adapter.read_effect_witness(handle, deadline), &before),
    };
    match changes {
        Some(changes) => match dispatch {
            Ok(result) => Ok(with_changes(result, &changes)),
            Err(error) => settle_unclear_answer(error, &changes, action_name),
        },
        None => dispatch.map(adapter_proof),
    }
}

/// The changes since `before`, or none when the witness could not be read.
/// A target that is gone after the action was closed or replaced by it.
fn after_changes(
    after: Result<EffectWitness, AdapterError>,
    before: &EffectWitness,
) -> Option<Vec<WitnessChange>> {
    match after {
        Ok(after) => Some(after.changes_since(before)),
        Err(error) if error.code == ErrorCode::StaleRef => Some(vec![WitnessChange::TargetGone]),
        Err(_) => None,
    }
}

fn adapter_proof(result: ActionResult) -> ActionResult {
    if result.disposition() == DeliverySemantics::delivered_verified() {
        result.with_effect(ObservedEffect::Observed)
    } else {
        result
    }
}

fn with_changes(result: ActionResult, changes: &[WitnessChange]) -> ActionResult {
    let effect = verdict(changes);
    let result = if effect == ObservedEffect::Observed {
        result.with_verified_delivery()
    } else {
        result
    };
    let mut result = result.with_effect(effect);
    if !changes.is_empty() {
        let details = result.details.get_or_insert_with(|| serde_json::json!({}));
        if let Some(details) = details.as_object_mut() {
            details.insert("witness_diff".into(), serde_json::json!(changes));
        }
    }
    result
}

/// An unclear answer is settled by what the witness saw. A strong change
/// proves the action landed, so the command succeeds as a verified delivery
/// and keeps the platform's report in `details`. Any other verdict leaves the
/// error as the platform reported it, with the verdict added.
fn settle_unclear_answer(
    mut error: AdapterError,
    changes: &[WitnessChange],
    action: &str,
) -> Result<ActionResult, AdapterError> {
    let effect = verdict(changes);
    if effect == ObservedEffect::Observed {
        let report = serde_json::json!({
            "code": error.code,
            "message": error.message,
            "details": error.details,
        });
        let result = ActionResult::delivered_unverified(action)
            .with_details(serde_json::json!({ "platform_report": report }));
        return Ok(with_changes(result, changes));
    }
    let details = error.details.get_or_insert_with(|| serde_json::json!({}));
    if let Some(details) = details.as_object_mut() {
        details.insert("effect".into(), serde_json::json!(effect));
        if !changes.is_empty() {
            details.insert("witness_diff".into(), serde_json::json!(changes));
        }
    }
    Err(error)
}

#[cfg(test)]
#[path = "witnessed_action_tests.rs"]
mod tests;
