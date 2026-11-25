//! Key delivery to a named window, gated on that window holding the focus.

use crate::{
    ActionResult, AdapterError, ErrorCode, InteractionLease, InteractionPolicy, KeyCombo,
    ProcessIdentity, WindowInfo, adapter::SystemOps,
};

/// The refusal for a key press whose named window could not be confirmed as
/// the focused one, with nothing sent.
fn exact_focus_unconfirmed(window: &WindowInfo, policy: InteractionPolicy) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The named window does not hold keyboard focus; no key was sent",
    )
    .with_suggestion(crate::response::headed_mode::window_focus_suggestion(
        policy,
    ))
    .with_details(serde_json::json!({
        "check": "window_active",
        "cause": "exact_window_focus_unconfirmed",
        "physical_delivery_started": false,
        "window_id": window.id,
    }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

/// Sends `combo` to the process of `window` only when `window` is the exact
/// focused window, and refuses before anything is sent otherwise. This is the
/// default of `SystemOps::press_key_for_window`.
pub(crate) fn press_key_in_focused_window<A: SystemOps + ?Sized>(
    adapter: &A,
    window: &WindowInfo,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    lease: &InteractionLease,
) -> Result<ActionResult, AdapterError> {
    let refusal = || exact_focus_unconfirmed(window, policy);
    let instance = window
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .ok_or_else(refusal)?;
    let focused = adapter
        .focused_window(lease.deadline())
        .map_err(AdapterError::unsent)?;
    if !focused.is_some_and(|focused| {
        focused.id == window.id
            && focused.pid == window.pid
            && focused.process_instance.as_deref() == Some(instance)
    }) {
        return Err(refusal());
    }
    adapter.press_key_for_app(
        ProcessIdentity::new(window.pid, instance),
        combo,
        policy,
        lease,
    )
}
