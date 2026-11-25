//! The witnessed ref-action path, kept beside [`super::helpers`] because it is
//! the same pipeline with an observer and the module has no room for both.

use crate::{ActionRequest, AppError, CommandContext, PlatformAdapter, RefEntry};
use serde_json::{Value, json};

use super::helpers::{
    RefArgs, apply_post_action_wait, load_ref_entry, post_delivery_error, update_lease_hold_ms,
    validate_post_action_wait,
};
use crate::ref_action::ref_action_wait_context::RefActionWaitContext;

/// The witnessed form of [`execute_ref_action_with_context`]: `witness` sees
/// this delivery from inside the lease the action ran under, and nothing it
/// does can change the result.
pub(crate) fn execute_ref_action_with_context_witnessed(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    request: ActionRequest,
    context: &CommandContext,
    witness: &dyn crate::ref_action::dispatch_witness::DispatchWitness,
) -> Result<Value, AppError> {
    let dispatch = crate::ref_action::ref_action_single::dispatch_resolved;
    let request = request.with_timeout_ms(args.timeout_ms);
    validate_post_action_wait(context)?;
    if request.policy.is_headed() && request.action.headed_requirement().requires_cursor() {
        super::point_resolve::require_pointer_capability(adapter)?;
    }
    let entry = load_ref_entry(&args.ref_id, context)?;
    let action_result =
        crate::ref_action::ref_action_wait::execute_with_auto_wait_and_lease_witnessed(
            RefActionWaitContext {
                adapter,
                entry: &entry,
                ref_id: &args.ref_id,
                context,
            },
            request,
            dispatch,
            witness,
        );
    finish(action_result, &args.ref_id, &entry, adapter, context)
}

pub(crate) fn finish(
    action_result: Result<
        (
            crate::ActionResult,
            crate::InteractionLease,
            crate::trace::trace_artifacts::ArtifactOutcome,
            crate::Deadline,
            std::time::Instant,
        ),
        crate::AdapterError,
    >,
    ref_id: &str,
    entry: &RefEntry,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (result, lease, pre, deadline, lease_started) = match action_result {
        Ok(result) => result,
        Err(mut error) => {
            if error.disposition.delivery() == crate::DeliveryDisposition::DeliveryUncertain
                && let Some(wait) = context.wait_selector()
            {
                let observation =
                    super::helpers::post_action_observation::observe(wait, entry, adapter);
                error = super::helpers::post_action_observation::attach(error, observation);
            }
            return Err(AppError::Adapter(error));
        }
    };
    let value = serde_json::to_value(result).map_err(|error| {
        post_delivery_error(AppError::Json(error), json!({ "action": "delivered" }))
    })?;
    let lease_hold_ms = u64::try_from(lease_started.elapsed().as_millis()).unwrap_or(u64::MAX);
    drop(lease);
    let mut outcome = apply_post_action_wait(value, Some(entry), adapter, context);
    update_lease_hold_ms(&mut outcome, lease_hold_ms);
    crate::ref_action::finish_artifacts(
        crate::ref_action::ref_action_context::RefActionContext::new(
            RefActionWaitContext {
                adapter,
                entry,
                ref_id,
                context,
            },
            deadline,
        ),
        &pre,
    );
    outcome
}
