//! Click.
//!
//! A list or tree item is clicked by selecting it: its LegacyIAccessible
//! default action is "Double Click" or "Expand", not a single press, and
//! Legacy `Select(SELFLAG_TAKESELECTION)` selects it with no foreground
//! change on SysTreeView32, WinForms, WPF and DirectUI items (C8). Every
//! other element is pressed through its LegacyIAccessible default action,
//! which changes no foreground state on Win32, WinForms, WPF and the Ribbon
//! (W-W0-6). InvokePattern activates the window and is the headed rung.

use agent_desktop_core::{ActionStep, AdapterError, Deadline, InteractionPolicy};
use std::time::Duration;

use crate::tree::element::UIAElement;

pub(crate) const SELECT_ITEM_LABEL: &str = "LegacyIAccessible.Select";

const VERIFY_TIMEOUT: Duration = Duration::from_millis(600);

const CLICK_SUGGESTION: &str = "Refresh the snapshot and retry, or target an element that advertises Invoke or a Legacy default action.";

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        ActionStep, AdapterError, CLICK_SUGGESTION, Deadline, InteractionPolicy, SELECT_ITEM_LABEL,
        UIAElement, VERIFY_TIMEOUT,
    };
    use crate::actions::chain::{
        INVOKE_LABEL, LEGACY_LABEL, after_delivery, invoke_available, invoke_pattern_delivered,
        legacy_default_action, legacy_default_action_available, legacy_select, offered, poll_until,
        rung,
    };
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use agent_desktop_core::DeliveryOutcome;
    use uiautomation::types::ControlType;

    pub(crate) fn click_steps(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let item = is_selectable_item(element);
        let mut select = || {
            if !legacy_select(element)? {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            let selected = poll_until(deadline, VERIFY_TIMEOUT, || {
                read_one(element, TreeProperty::SelectionItemIsSelected).flag() == Some(true)
            })
            .map_err(after_delivery)?;
            Ok(DeliveryOutcome::from_delivery(true, selected))
        };
        let mut press = || {
            Ok(DeliveryOutcome::from_delivery(
                legacy_default_action(element)?,
                false,
            ))
        };
        let mut invoke = || {
            Ok(DeliveryOutcome::from_delivery(
                invoke_pattern_delivered(element)?,
                false,
            ))
        };
        agent_desktop_core::execute_chain(
            &mut offered([
                rung(SELECT_ITEM_LABEL, item, false, &mut select),
                rung(
                    LEGACY_LABEL,
                    !item && legacy_default_action_available(element),
                    false,
                    &mut press,
                ),
                rung(INVOKE_LABEL, invoke_available(element), true, &mut invoke),
            ]),
            policy,
            deadline,
            CLICK_SUGGESTION,
        )
    }

    fn is_selectable_item(element: &UIAElement) -> bool {
        let control_type = read_one(element, TreeProperty::ControlType).number();
        matches!(control_type, Some(id) if id == ControlType::ListItem as i32 || id == ControlType::TreeItem as i32)
            && read_one(element, TreeProperty::SelectionItemAvailable).flag() == Some(true)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionStep, AdapterError, Deadline, InteractionPolicy, UIAElement};

    pub(crate) fn click_steps(
        _: &UIAElement,
        _: InteractionPolicy,
        _: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Click"))
    }
}

pub(crate) use imp::click_steps;
