//! Key delivery to a named window, gated on that window holding the focus.

use crate::adapter::SystemOps;
use crate::{
    ActionResult, AdapterError, ErrorCode, InteractionLease, InteractionPolicy, KeyCombo,
    ProcessIdentity, WindowInfo,
};

/// Delivers `combo` to the process of `window` only after the window reads
/// back as the focused one with the stored process instance.
pub(super) fn press_in_exact_window<A: SystemOps + ?Sized>(
    adapter: &A,
    window: &WindowInfo,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    lease: &InteractionLease,
) -> Result<ActionResult, AdapterError> {
    let refusal = || {
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
    };
    let instance = window
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .ok_or_else(|| {
            AdapterError::stale_ref_because("The named window has no process identity")
                .with_suggestion("Run 'list-windows' again and use the window's current id")
        })?;
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
