use agent_desktop_core::{AdapterError, CursorOverlayControl, Deadline, Point};
use serde_json::{Value, json};

use super::extension_request::{cursor_request, travel_destination};
use super::extension_start::CursorStart;
use super::{Unavailable, call};

/// The refusal for an overlay the extension cannot draw. It keeps the code and
/// `rendered: false` an unrendered Wayland overlay has always had, and names
/// why in `cause`.
pub(crate) fn unrendered(unavailable: Unavailable) -> AdapterError {
    AdapterError::not_supported("cursor overlay on Wayland")
        .with_suggestion(unavailable.suggestion())
        .with_details(json!({"rendered": false, "cause": unavailable.cause()}))
}

fn send(request: &Value, deadline: Deadline) -> Result<Result<(), Unavailable>, AdapterError> {
    call::<_, ()>("UpdateCursor", &(request.to_string(),), deadline)
}

fn locate(
    control: &CursorOverlayControl,
    destination: &Point,
    deadline: Deadline,
) -> Result<Result<CursorStart, Unavailable>, AdapterError> {
    let request = json!({
        "session": control.session_id(),
        "agent": control.agent_id(),
        "destination": {"x": destination.x, "y": destination.y},
    });
    let reply = match call::<_, String>("Locate", &(request.to_string(),), deadline)? {
        Ok(reply) => reply,
        Err(unavailable) => return Ok(Err(unavailable)),
    };
    serde_json::from_str(&reply).map(Ok).map_err(|error| {
        AdapterError::internal("The agent-desktop GNOME Shell extension answered malformed data")
            .with_platform_detail(error.to_string())
    })
}

/// Draws one overlay control through the extension. A travel first asks where
/// the cursor stands, so the path is core's motion from there; the extension
/// answers the travel once the cursor has arrived.
pub(crate) fn update_cursor(
    control: &CursorOverlayControl,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    control.validate()?;
    let start = match control.instruction().filter(|_| control.is_travel()) {
        Some(instruction) => {
            Some(locate(control, &travel_destination(instruction), deadline)?.map_err(unrendered)?)
        }
        None => None,
    };
    send(&cursor_request(control, start), deadline)?.map_err(unrendered)
}

/// Runs `capture` with every agent cursor hidden, so a screenshot shows the
/// desktop without them, and shows them again afterwards. Without the
/// extension there is nothing drawn to hide. The extension shows them again by
/// itself if this process never asks.
pub(crate) fn with_concealed<T>(
    deadline: Deadline,
    capture: impl FnOnce() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    let concealed = matches!(send(&json!({"op": "conceal"}), deadline)?, Ok(()));
    let result = capture();
    if concealed {
        let cleanup = Deadline::detached_after(1_500)?;
        send(&json!({"op": "reveal"}), cleanup)?.map_err(unrendered)?;
    }
    result
}

#[cfg(test)]
#[path = "extension_cursor_tests.rs"]
mod tests;
