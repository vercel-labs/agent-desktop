//! `execute_action` match over every `Action` variant.
//!
//! Click routes through `click`. SetValue / Clear route
//! through `value_write`. Toggle / Check / Uncheck and Expand / Collapse route
//! through `toggle_state` and `disclosure`. Select and Scroll route through
//! their dedicated modules. SetFocus routes through `focus`.
//! ScrollTo runs the ScrollIntoView spine plus the ancestor ladder when
//! ScrollItem is absent or leaves geometry unchanged. Physical keyboard and
//! click legs route through `physical_keyboard` and `physical_click`.

use agent_desktop_core::{
    Action, ActionResult, AdapterError, Deadline, ErrorCode, InteractionLease, NativeHandle,
    actions::action_request::ActionRequest,
};

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        Action, ActionRequest, ActionResult, AdapterError, Deadline, ErrorCode, InteractionLease,
        NativeHandle,
    };
    use crate::actions::chain::build_step;
    use crate::actions::click::click_steps;
    use crate::actions::disclosure::disclosure_steps;
    use crate::actions::focus::focus_element;
    use crate::actions::physical_click::{
        double_click_steps, right_click_steps, triple_click_steps,
    };
    use crate::actions::physical_keyboard::{
        press_key_element_steps, press_key_global, type_text_steps,
    };
    use crate::actions::scroll::scroll_steps;
    use crate::actions::scroll_into_view::scroll_into_view_outcome;
    use crate::actions::select::select_steps;
    use crate::actions::toggle_state::{ToggleGoal, toggle_steps};
    use crate::actions::value_write::{clear_steps, set_value_steps};
    use crate::tree::element::uia_element;
    use crate::tree::live_read::corroborate_verified_process;

    pub(crate) fn execute_action_impl(
        handle: &NativeHandle,
        request: ActionRequest,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        let deadline = lease.deadline();
        if handle.is_null() {
            return null_handle_action(&request.action, deadline);
        }
        let element = uia_element(handle)?;
        deadline.ensure_remaining()?;
        corroborate_verified_process(element)?;
        match &request.action {
            Action::Click => {
                let steps = click_steps(element, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::SetFocus => focus_element(element, &request, deadline),
            Action::ScrollTo => execute_scroll_to(handle, lease, request.policy),
            Action::TypeText(text) => {
                let steps = type_text_steps(element, text, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::PressKey(combo) => {
                let steps = press_key_element_steps(element, combo, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::DoubleClick => {
                let steps = double_click_steps(
                    element,
                    request.policy,
                    deadline,
                    request.verified_point().cloned(),
                )?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::TripleClick => {
                let steps = triple_click_steps(
                    element,
                    request.policy,
                    deadline,
                    request.verified_point().cloned(),
                )?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::RightClick => {
                let steps = right_click_steps(
                    element,
                    request.policy,
                    deadline,
                    request.verified_point().cloned(),
                )?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::KeyDown(_) | Action::KeyUp(_) | Action::Hover | Action::Drag(_) => {
                adapter_level_rejection(request.action.name())
            }
            Action::SetValue(value) => {
                let steps = set_value_steps(element, value, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::Clear => {
                let steps = clear_steps(element, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::Toggle => {
                let steps = toggle_steps(element, ToggleGoal::Flip, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::Check => {
                let steps =
                    toggle_steps(element, ToggleGoal::Checked(true), request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::Uncheck => {
                let steps = toggle_steps(
                    element,
                    ToggleGoal::Checked(false),
                    request.policy,
                    deadline,
                )?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::Expand => {
                let steps = disclosure_steps(element, true, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::Collapse => {
                let steps = disclosure_steps(element, false, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::Select(value) => {
                let steps = select_steps(element, value, request.policy, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
            Action::Scroll(direction, amount) => {
                let steps = scroll_steps(element, direction, *amount, deadline)?;
                Ok(ActionResult::from_execution(&request.action, steps))
            }
        }
    }

    fn null_handle_action(
        action: &Action,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        match action {
            Action::PressKey(combo) => press_key_global(combo, deadline),
            _ => Err(invalid_native_handle(true)),
        }
    }

    fn invalid_native_handle(empty: bool) -> AdapterError {
        AdapterError::new(
            ErrorCode::InvalidArgs,
            "Native handle does not contain a Windows UI Automation element",
        )
        .with_details(serde_json::json!({
            "kind": "invalid_native_handle",
            "platform": "windows",
            "empty": empty
        }))
    }

    fn adapter_level_rejection(label: &str) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            format!("{label} requires adapter-level handling, not element action"),
        )
        .with_suggestion(
            "Use the top-level command (e.g. 'hover', 'drag', 'press') instead of targeting an element.",
        ))
    }

    fn execute_scroll_to(
        handle: &NativeHandle,
        lease: &InteractionLease,
        policy: agent_desktop_core::InteractionPolicy,
    ) -> Result<ActionResult, AdapterError> {
        let done = scroll_into_view_outcome(handle, lease, policy)?;
        Ok(ActionResult::from_execution(
            &Action::ScrollTo,
            vec![build_step(done.label, done.outcome)],
        ))
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionRequest, ActionResult, AdapterError, InteractionLease, NativeHandle};

    pub(crate) fn execute_action_impl(
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("execute_action"))
    }
}

pub(crate) use imp::execute_action_impl;

#[cfg(all(test, target_os = "windows"))]
#[path = "dispatch_tests.rs"]
mod tests;
