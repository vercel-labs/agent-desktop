//! `SetFocus` with focused-element verification.
//!
//! UIA `SetFocus` moves the desktop foreground window (A3-4, A19-5), so it is
//! a headed rung and headless dispatch is refused with `POLICY_DENIED`.
//! Headed dispatch verifies by comparing the automation client's focused
//! element to the target.

use agent_desktop_core::{
    Action, ActionResult, AdapterError, Deadline, actions::action_request::ActionRequest,
};

#[cfg(target_os = "windows")]
mod imp {
    use super::{Action, ActionRequest, ActionResult, AdapterError, Deadline};
    use crate::actions::chain::{offered, rung};
    use crate::actions::mutation::{classify_mutation, classify_success};
    use crate::tree::automation::{automation_client, failure_of};
    use crate::tree::element::UIAElement;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use crate::tree::walk::walker_source::same_element;
    use agent_desktop_core::DeliveryOutcome;

    const SET_FOCUS_API: &str = "UIElement.SetFocus";
    const SET_FOCUS_OP: &str = "SetFocus";
    const FOCUS_SUGGESTION: &str = "SetFocus is not available on this element.";

    pub(crate) fn focus_element(
        element: &UIAElement,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let mut set_focus = || {
            let delivered = invoke_focus(element)?;
            Ok(DeliveryOutcome::from_delivery(
                delivered,
                delivered && focus_matches_target(element)?,
            ))
        };
        let steps = agent_desktop_core::execute_chain(
            &mut offered([rung(SET_FOCUS_API, true, true, &mut set_focus)]),
            request.policy,
            deadline,
            FOCUS_SUGGESTION,
        )?;
        Ok(ActionResult::from_execution(&Action::SetFocus, steps))
    }

    fn invoke_focus(element: &UIAElement) -> Result<bool, AdapterError> {
        match element.0.set_focus() {
            Ok(()) => classify_success(),
            Err(error) => classify_mutation(SET_FOCUS_OP, SET_FOCUS_API, &failure_of(&error)),
        }
    }

    fn focus_matches_target(element: &UIAElement) -> Result<bool, AdapterError> {
        let client = automation_client()?;
        match client.get_focused_element() {
            Ok(focused) => Ok(same_element(&client, element, &UIAElement::from(focused))),
            Err(_) => Ok(false),
        }
    }

    pub(crate) fn keyboard_focus_ready(element: &UIAElement) -> Result<bool, AdapterError> {
        if read_one(element, TreeProperty::HasKeyboardFocus).flag() == Some(true) {
            return Ok(true);
        }
        focus_matches_target(element)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionRequest, ActionResult, AdapterError, Deadline};
    use crate::tree::element::UIAElement;

    pub(crate) fn focus_element(
        _element: &UIAElement,
        _request: &ActionRequest,
        _deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("SetFocus"))
    }
}

pub(crate) use imp::focus_element;

#[cfg(target_os = "windows")]
pub(crate) use imp::keyboard_focus_ready;
