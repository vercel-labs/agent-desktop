//! The route of a plain Escape that the key route refused for focus.
//!
//! The key route sends a key only to a target that holds the keyboard. A
//! menu, a drop-down or a popup can stay open in a window that does not, and
//! the key is then refused while the surface stays on the user's screen. The
//! states, in this order: the key route's result stands when it delivered or
//! refused for any other reason; after a refusal for focus, the adapter
//! closes the open transient surface of the target through the platform's
//! own close action, and that result stands, a failure of that attempt
//! included, because the attempt's own error says whether it may have
//! landed; the refusal stands only when no surface is open or the adapter
//! has no close route.

use crate::{Action, ActionResult, AdapterError, DeliverySemantics, ErrorCode, KeyCombo};

/// The `details.check` values with which an adapter refuses a key because
/// the target window or process does not hold the keyboard.
const FOCUS_CHECKS: [&str; 3] = ["window_active", "exact_window_focus", "process_focus"];

pub(crate) fn is_plain_escape(combo: &KeyCombo) -> bool {
    combo.modifiers.is_empty() && matches!(combo.key.as_str(), "escape" | "esc")
}

pub(crate) fn refused_for_focus(error: &AdapterError) -> bool {
    error.disposition == DeliverySemantics::not_delivered()
        && error
            .details
            .as_ref()
            .and_then(|details| details.get("check"))
            .and_then(|check| check.as_str())
            .is_some_and(|check| FOCUS_CHECKS.contains(&check))
}

/// The key route's result, or the surface close after a refusal for focus.
pub(crate) fn settle(
    keyed: Result<ActionResult, AdapterError>,
    combo: &KeyCombo,
    close: impl FnOnce() -> Result<Option<ActionResult>, AdapterError>,
) -> Result<ActionResult, AdapterError> {
    match keyed {
        Err(refusal) if is_plain_escape(combo) && refused_for_focus(&refusal) => match close() {
            Ok(Some(mut closed)) => {
                closed.action = Action::PressKey(combo.clone()).name().into();
                Ok(closed)
            }
            Ok(None) => Err(refusal),
            Err(attempt) if attempt.code == ErrorCode::PlatformNotSupported => Err(refusal),
            Err(attempt) => Err(attempt),
        },
        keyed => keyed,
    }
}

#[cfg(test)]
#[path = "escape_route_tests.rs"]
mod tests;
