//! The one foreground gate. An adapter reports what an operation needs from
//! the foreground; the interaction policy decides whether it may take it. The
//! actionability preflight, the raw native-handle dispatch and `close-app` all
//! decide through here.

use serde_json::json;

use crate::{
    AdapterError, Deadline, HeadedRequirement, InteractionPolicy,
    actions::action_request::ActionRequest,
    adapter::{NativeHandle, PlatformAdapter},
};

/// Whether `policy` forbids the focus or the cursor that `requirement` needs.
pub(crate) fn denies(requirement: HeadedRequirement, policy: InteractionPolicy) -> bool {
    requirement.requires_focus() && !policy.allow_focus_steal
        || requirement.requires_cursor() && !policy.allow_cursor_move
}

/// The refusal for an operation whose foreground use `policy` forbids. `kind`
/// names the effect in `details.kind`; nothing was sent.
pub fn foreground_denied(message: &str, kind: &str, policy: InteractionPolicy) -> AdapterError {
    AdapterError::policy_denied_for_policy(message, policy)
        .with_details(json!({ "check": "foreground", "kind": kind }))
}

/// The gate for a dispatch that runs without the actionability preflight, such
/// as the raw native-handle call of the C ABI.
pub fn ensure_foreground_allowed(
    adapter: &dyn PlatformAdapter,
    handle: &NativeHandle,
    request: &ActionRequest,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let requirement = adapter.headed_requirement(handle, request, deadline)?;
    if denies(requirement, request.policy) {
        return Err(foreground_denied(
            "The platform reports that this action takes the foreground, which the interaction policy does not allow",
            "action",
            request.policy,
        ));
    }
    Ok(())
}
