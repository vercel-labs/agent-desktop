use agent_desktop_core::{ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode};

use crate::tree::AXElement;

use super::{CheckTarget, after_delivery, value_write_step};

pub(super) fn refuse_uncheck(
    target: &mut impl CheckTarget,
    want_checked: bool,
    write_refused: bool,
) -> Result<Vec<ActionStep>, AdapterError> {
    if !write_refused {
        return Err(uncheck_unsupported());
    }
    match target.checked().map_err(after_delivery)? {
        Some(state) if state == want_checked => Ok(vec![value_write_step()]),
        Some(_) => Err(uncheck_unsupported()),
        None => Err(after_delivery(uncheck_unsupported())),
    }
}

pub(super) fn uncheck_unsupported() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "a radio button cannot be unchecked directly",
    )
    .with_details(serde_json::json!({
        "verification": "requested_checked_state_not_observed"
    }))
    .with_suggestion(
        "A radio button cannot be unchecked directly. Select a sibling radio button in the same group instead.",
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

/// Reports whether check/uncheck may write `AXValue` on an element with this subrole.
///
/// A tab button (`AXRadioButton` with subrole `AXTabButton`) is selected, not checked.
/// Chromium-based browsers report `AXValue` as settable on a web `role="tab"` element,
/// and writing it crashes the browser, so check/uncheck presses tab buttons instead.
pub(super) fn value_write_allowed(subrole: Option<&str>) -> bool {
    subrole != Some("AXTabButton")
}

/// Reads the element's `AXSubrole`.
pub(super) fn subrole(el: &AXElement, deadline: Deadline) -> Result<Option<String>, AdapterError> {
    crate::tree::resolve::resolve_ax_read::read_string(el, "AXSubrole", deadline)
}
