use agent_desktop_core::{
    Action, ActionResult, ActionStep, AdapterError, Deadline, ErrorCode, StepMechanism,
    actions::action_request::ActionRequest,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::{
        chain::chain_run::run_chain,
        chain::{ChainContext, chain_defs},
        toggle_state,
    };
    use crate::tree::AXElement;

    const DISCLOSURE_SUGGESTION: &str = "Target a control with a readable expandable state.";
    const MULTI_CLICK_SUGGESTION: &str = "Use a headed physical double-click or triple-click; the semantic routes cannot repeat a click.";

    pub(crate) fn perform_action(
        el: &AXElement,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let ctx = ChainContext {
            dynamic_value: None,
            verified_point: request.verified_point(),
            deadline,
            policy: request.policy,
        };
        crate::tree::properties::attributes::set_messaging_timeout(el, deadline)?;
        deadline.ensure_remaining()?;
        let action = &request.action;
        let label = action.name();
        let mut steps = Vec::new();
        tracing::debug!("action: perform {label}");
        match action {
            Action::Click => {
                let native_menu = crate::tree::native_menu::presses_like_a_menu(el, deadline)?
                    || (request.policy.is_headed()
                        && probe_click_role(el, deadline)?.as_deref() == Some("AXMenuBarItem"));
                let suggestion = if native_menu {
                    "Refresh the menu snapshot and target an item that supports AXPress."
                } else {
                    "Target an element that advertises Click or use an explicit point click."
                };
                steps.extend(run_chain(
                    el,
                    &ctx,
                    chain_defs::click_rungs(el, &ctx, native_menu),
                    suggestion,
                )?);
            }

            Action::DoubleClick => {
                steps.extend(run_chain(
                    el,
                    &ctx,
                    chain_defs::multi_click_rungs(el, &ctx, 2),
                    MULTI_CLICK_SUGGESTION,
                )?);
            }

            Action::RightClick => {
                steps.extend(run_chain(
                    el,
                    &ctx,
                    chain_defs::right_click_rungs(el, &ctx),
                    "Try 'mouse-click --button right --xy X,Y'.",
                )?);
            }

            Action::Toggle => {
                steps.extend(toggle_state::toggle(el, request.policy, deadline)?);
            }

            Action::SetValue(val) => {
                crate::actions::chain::chain_value_write::refuse_file_name_field(el, deadline)?;
                let valued = ChainContext {
                    dynamic_value: Some(val.as_str()),
                    ..ctx
                };
                steps.extend(run_chain(
                    el,
                    &valued,
                    chain_defs::set_value_rungs(el, &valued),
                    "Target an element with a settable value or native increment/decrement actions.",
                )?);
            }

            Action::SetFocus => {
                steps.extend(run_chain(
                    el,
                    &ctx,
                    chain_defs::focus_rungs(el, &ctx),
                    "Target an element whose AXFocused attribute is settable.",
                )?);
            }

            Action::TypeText(text) => {
                if !request.policy.is_headed() {
                    crate::actions::chain::chain_value_write::refuse_file_name_field(el, deadline)?;
                }
                steps.push(crate::actions::type_text::execute_type(
                    el,
                    text.as_str(),
                    request.policy,
                    deadline,
                )?);
            }

            Action::PressKey(combo) => {
                crate::actions::physical_keyboard::press(el, combo, request.policy, deadline)?;
                steps.push(
                    ActionStep::succeeded("PressKey")
                        .with_mechanism(StepMechanism::PhysicalSynthetic)
                        .with_verified(false),
                );
            }

            Action::Expand => {
                steps.extend(run_chain(
                    el,
                    &ctx,
                    chain_defs::expand_rungs(el, &ctx),
                    DISCLOSURE_SUGGESTION,
                )?);
            }

            Action::Collapse => {
                steps.extend(run_chain(
                    el,
                    &ctx,
                    chain_defs::collapse_rungs(el, &ctx),
                    DISCLOSURE_SUGGESTION,
                )?);
            }

            Action::Select(value) => {
                let verified = crate::actions::extras::select_value(
                    el,
                    value.as_str(),
                    request.policy,
                    deadline,
                )?;
                steps.push(
                    ActionStep::succeeded("Select")
                        .with_mechanism(StepMechanism::SemanticApi)
                        .with_verified(verified),
                );
            }

            Action::Scroll(direction, amount) => {
                let (mechanism, verified) = crate::actions::scroll::ax_scroll(
                    el,
                    direction,
                    *amount,
                    request.policy,
                    deadline,
                )?;
                steps.push(
                    ActionStep::succeeded("Scroll")
                        .with_mechanism(mechanism)
                        .with_verified(verified),
                );
            }

            Action::Check => {
                steps.extend(toggle_state::check_uncheck(
                    el,
                    true,
                    request.policy,
                    deadline,
                )?);
            }

            Action::Uncheck => {
                steps.extend(toggle_state::check_uncheck(
                    el,
                    false,
                    request.policy,
                    deadline,
                )?);
            }

            Action::TripleClick => {
                steps.extend(run_chain(
                    el,
                    &ctx,
                    chain_defs::multi_click_rungs(el, &ctx, 3),
                    MULTI_CLICK_SUGGESTION,
                )?);
            }

            Action::ScrollTo => {
                steps.extend(run_chain(
                    el,
                    &ctx,
                    chain_defs::scroll_to_rungs(el, &ctx),
                    "Target an element that advertises AXScrollToVisible.",
                )?);
            }

            Action::Clear => {
                crate::actions::chain::chain_value_write::refuse_file_name_field(el, deadline)?;
                let cleared = ChainContext {
                    dynamic_value: Some(""),
                    ..ctx
                };
                steps.extend(run_chain(
                    el,
                    &cleared,
                    chain_defs::clear_rungs(el, &cleared),
                    "Target an editable control or allow the verified keyboard fallback.",
                )?);
            }

            Action::KeyDown(_) | Action::KeyUp(_) | Action::Hover | Action::Drag(_) => {
                return Err(AdapterError::new(
                    ErrorCode::ActionNotSupported,
                    format!(
                        "{} requires adapter-level handling, not element action",
                        label
                    ),
                )
                .with_suggestion("Use the top-level command (e.g. 'hover', 'drag', 'press') instead of targeting an element."));
            }
        }

        Ok(ActionResult::from_execution(action, steps))
    }

    fn probe_click_role(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<Option<String>, AdapterError> {
        let operation_deadline = deadline;
        match crate::tree::resolve::resolve_ax_read::read_string(
            element,
            "AXRole",
            operation_deadline,
        ) {
            Ok(Some(role))
                if role == "AXMenuBarItem"
                    && crate::tree::shell_surface::is_status_item(element, operation_deadline) =>
            {
                Ok(None)
            }
            Err(error) if error.code == ErrorCode::AppUnresponsive => Ok(None),
            result => result,
        }
    }
}

pub(crate) use imp::perform_action;
