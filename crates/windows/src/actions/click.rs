//! Click.
//!
//! A tab item of a native tab control is clicked through `TCM_SETCURFOCUS`
//! (`select_tab`). A list or tree item is clicked by selecting it: its LegacyIAccessible
//! default action is "Double Click" or "Expand", not a single press, and
//! Legacy `Select(SELFLAG_TAKESELECTION)` selects it with no foreground
//! change on SysTreeView32, WinForms, WPF and DirectUI items (C8). Every
//! other element is pressed through its LegacyIAccessible default action,
//! which changes no foreground state on Win32, WinForms, WPF and the Ribbon
//! (W-W0-6). InvokePattern takes no foreground either (A31-5), so every
//! policy runs it. A grid cell with none of those routes, the one the snapshot
//! advertises `Click` for, is selected through the item that holds it
//! (`click_container`), the last semantic rung.

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
        INVOKE_LABEL, LEGACY_LABEL, SELECTION_ITEM_LABEL, invoke_available,
        invoke_pattern_delivered, legacy_default_action, legacy_default_action_available,
        legacy_select, offered, poll_until, rung, selection_item_select,
    };
    use crate::actions::click_container::clicked_item;
    use crate::actions::select_tab::{TAB_SELECT_LABEL, is_tab_item, select_native_tab};
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
        let tab_item = is_tab_item(element);
        let default_action = legacy_default_action_available(element);
        let invokable = invoke_available(element);
        let container = if tab_item || item || default_action || invokable {
            None
        } else {
            clicked_item(element, deadline)
        };
        let mut select = || {
            if !legacy_select(element)? {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            let selected = poll_until(deadline, VERIFY_TIMEOUT, || {
                read_one(element, TreeProperty::SelectionItemIsSelected).flag() == Some(true)
            });
            Ok(DeliveryOutcome::from_delivery(true, selected))
        };
        let mut tab =
            || Ok(select_native_tab(element, deadline)?.unwrap_or(DeliveryOutcome::NotDelivered));
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
        let mut select_container = || {
            let Some(target) = container.as_ref() else {
                return Ok(DeliveryOutcome::NotDelivered);
            };
            if !selection_item_select(target)? {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            let selected = poll_until(deadline, VERIFY_TIMEOUT, || {
                read_one(target, TreeProperty::SelectionItemIsSelected).flag() == Some(true)
            });
            Ok(DeliveryOutcome::from_delivery(true, selected))
        };
        agent_desktop_core::execute_chain(
            &mut offered([
                rung(TAB_SELECT_LABEL, tab_item, false, &mut tab),
                rung(SELECT_ITEM_LABEL, item, false, &mut select),
                rung(LEGACY_LABEL, !item && default_action, false, &mut press),
                rung(INVOKE_LABEL, invokable, false, &mut invoke),
                rung(
                    SELECTION_ITEM_LABEL,
                    container.is_some(),
                    false,
                    &mut select_container,
                ),
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
