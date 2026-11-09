//! The one foreground gate. An adapter declares the effects an operation has
//! on the user's foreground; the interaction policy decides whether it may have
//! them. The actionability preflight, the raw native-handle dispatch and
//! `close-app` all decide through here.

use serde_json::json;

use crate::{
    AdapterError, Deadline, DeclaredEffect, Effect, InteractionPolicy,
    actions::action_request::ActionRequest,
    adapter::{NativeHandle, PlatformAdapter},
};

/// The refusal for `mechanism`, which takes the foreground `policy` forbids.
/// `kind` says why the platform expects it to; nothing was sent.
pub fn foreground_denied(
    message: &str,
    kind: &str,
    mechanism: &str,
    policy: InteractionPolicy,
) -> AdapterError {
    AdapterError::policy_denied_for_policy(message, policy).with_details(json!({
        "check": "foreground",
        "kind": kind,
        "effect": Effect::FocusSteal,
        "foreground_effect": true,
        "mechanisms": [mechanism],
    }))
}

/// Refuses `request` when its policy does not permit the effect the platform
/// declared for it, and reports the declared reason as `details.kind`.
pub(crate) fn require_declared(
    declared: Option<DeclaredEffect>,
    request: &ActionRequest,
) -> Result<(), AdapterError> {
    let Some(declared) = declared else {
        return Ok(());
    };
    let operation = request.action.name();
    request
        .policy
        .require(declared.effect, operation)
        .map_err(|refusal| {
            refusal
                .with_suggestion(crate::headed_mode_suggestion(
                    "opens a popup, menu or window that takes the foreground",
                ))
                .with_details(json!({
                    "check": "foreground",
                    "kind": declared.kind,
                    "foreground_effect": true,
                    "mechanisms": [operation],
                }))
        })
}

/// The gate for a dispatch that runs without the actionability preflight, such
/// as the raw native-handle call of the C ABI.
pub fn ensure_foreground_allowed(
    adapter: &dyn PlatformAdapter,
    handle: &NativeHandle,
    request: &ActionRequest,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    require_declared(
        adapter.foreground_effect(handle, request, deadline)?,
        request,
    )
}
