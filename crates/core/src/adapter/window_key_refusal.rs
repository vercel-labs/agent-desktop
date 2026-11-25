use crate::adapter::SystemOps;
use crate::{
    ActionResult, AdapterError, InteractionLease, InteractionPolicy, KeyCombo, ProcessIdentity,
    WindowInfo,
};

/// The refusal for a key press whose named window could not be confirmed as
/// the focused one, with nothing sent.
pub(super) fn exact_focus_unconfirmed(
    window: &WindowInfo,
    policy: InteractionPolicy,
) -> AdapterError {
    AdapterError::stale_ref_because(
        "The named window's exact focus could not be confirmed, so no key was sent",
    )
    .with_suggestion(if policy.is_headed() {
        "Ensure the named window remains active before retrying; a sibling window or another application's menu can hold the keyboard".to_owned()
    } else {
        format!("{} Or name the already active window.", crate::headed_mode_suggestion("brings the named window to the foreground before sending keys"))
    })
    .with_details(serde_json::json!({
        "check": "window_active",
        "cause": "exact_window_focus_unconfirmed",
        "physical_delivery_started": false,
        "window_id": window.id,
    }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

/// Delivers `combo` to the process of `window` only after the window reads
/// back as the focused one with the stored process instance.
pub(super) fn press_in_exact_window<A: SystemOps + ?Sized>(
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
