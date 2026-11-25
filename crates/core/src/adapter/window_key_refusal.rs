use crate::{AdapterError, InteractionPolicy, WindowInfo};

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
