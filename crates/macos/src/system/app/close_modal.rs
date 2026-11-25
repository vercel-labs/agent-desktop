//! A prompt that a close request cannot get past.
//!
//! An alert or a sheet that is open (an unsaved-changes sheet, a failed-open
//! alert) is modal: the application answers a close request by waiting on it,
//! so the process stays up until a person answers. Reading it before the
//! request keeps a request that would only queue behind the prompt from being
//! sent. Reading it while the exit is awaited names the prompt the request
//! raised, so the wait ends on what blocks it instead of on the deadline.

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, SurfaceInfo};

pub(crate) fn pending_modal(surfaces: &[SurfaceInfo]) -> Option<&SurfaceInfo> {
    surfaces
        .iter()
        .find(|surface| matches!(surface.kind.as_str(), "sheet" | "alert"))
}

/// The refusal for an application held by a prompt. `sent` says whether the
/// close request went out before the prompt was seen, which is what raised it.
pub(crate) fn modal_pending(id: &str, pid: i32, surface: &SurfaceInfo, sent: bool) -> AdapterError {
    let named = surface.title.as_deref().map_or_else(
        || surface.kind.clone(),
        |title| format!("{} '{title}'", surface.kind),
    );
    let (message, delivery) = if sent {
        (
            format!("The close request for '{id}' is waiting on an open {named}"),
            DeliverySemantics::delivered_unverified(),
        )
    } else {
        (
            format!("'{id}' has an open {named}, so no close request was sent"),
            DeliverySemantics::not_delivered(),
        )
    };
    AdapterError::new(ErrorCode::ActionFailed, message)
        .with_suggestion(
            "Answer the prompt in the app, or use --force, which ends the app and discards unsaved work.",
        )
        .with_details(serde_json::json!({
            "cause": "modal_pending",
            "app": id,
            "pid": pid,
            "surface": surface.kind,
            "title": surface.title,
        }))
        .with_disposition(delivery)
}

#[cfg(test)]
#[path = "close_modal_tests.rs"]
mod tests;
