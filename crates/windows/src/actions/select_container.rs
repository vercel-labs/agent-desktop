//! Finding a named item through UI Automation's own virtualization contract.
//!
//! A container that offers `ItemContainerPattern` answers
//! `FindItemByProperty(Name)` for an item it has not built yet, and the item's
//! `VirtualizedItemPattern.Realize` builds it. No scroll is driven from here,
//! so it reaches the items of a XAML combo box whose own `ScrollPattern` is
//! absent (the Settings "Country or region" combo, probe 2026-11-07), and it
//! decides only by the patterns the element exposes. Every call here is a
//! control pattern call, so the caller runs it only where the policy allows a
//! foreground effect. `Realize` returns before the item is usable, so the item
//! is looked up again after it and waited for until it is on screen; the
//! reference from before `Realize` is never selected.

use agent_desktop_core::{AdapterError, Deadline};

use crate::tree::element::UIAElement;

/// Whether the container route runs: nothing was found by walking the tree and
/// the policy allows a pattern call that may activate the window.
pub(crate) fn container_route_applies(already_found: bool, allow_focus_steal: bool) -> bool {
    !already_found && allow_focus_steal
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{AdapterError, Deadline, UIAElement};
    use crate::actions::chain::poll_until;
    use crate::actions::select_search::{accept_selection_match, selection_item_available};
    use crate::tree::automation::automation_client;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use crate::tree::walk::walker_source::same_element;
    use std::time::Duration;
    use uiautomation::variants::Variant;
    use windows::Win32::UI::Accessibility::{
        IUIAutomationElement, IUIAutomationItemContainerPattern,
        IUIAutomationVirtualizedItemPattern, UIA_ItemContainerPatternId, UIA_NamePropertyId,
        UIA_VirtualizedItemPatternId,
    };

    const READY_TIMEOUT: Duration = Duration::from_millis(800);

    /// The realized item named exactly `value` in `container`, or `None` when
    /// the container offers no `ItemContainerPattern`, does not know the name,
    /// or cannot realize it. A second item with the same name is
    /// `AMBIGUOUS_TARGET`, as in the tree walk.
    pub(crate) fn realize_named_item(
        container: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<Option<UIAElement>, AdapterError> {
        deadline.ensure_remaining()?;
        let raw: &IUIAutomationElement = container.0.as_ref();
        let Ok(pattern) = (unsafe {
            raw.GetCurrentPatternAs::<IUIAutomationItemContainerPattern>(UIA_ItemContainerPatternId)
        }) else {
            return Ok(None);
        };
        let name = Variant::from(value);
        let Some(first) = find_after(&pattern, None, &name) else {
            return Ok(None);
        };
        let first = UIAElement::from(uiautomation::UIElement::from(first));
        if let Some(second) = find_after(&pattern, Some(first.0.as_ref()), &name) {
            let second = UIAElement::from(uiautomation::UIElement::from(second));
            if !same_element(&automation_client()?, &first, &second) {
                accept_selection_match(true)?;
            }
        }
        deadline.ensure_remaining()?;
        if let Ok(virtualized) = unsafe {
            first
                .0
                .as_ref()
                .GetCurrentPatternAs::<IUIAutomationVirtualizedItemPattern>(
                    UIA_VirtualizedItemPatternId,
                )
        } && unsafe { virtualized.Realize() }.is_err()
        {
            return Ok(None);
        }
        let item = match find_after(&pattern, None, &name) {
            Some(again) => UIAElement::from(uiautomation::UIElement::from(again)),
            None => first,
        };
        poll_until(deadline, READY_TIMEOUT, || {
            read_one(&item, TreeProperty::IsOffscreen).flag() == Some(false)
        })?;
        Ok(selection_item_available(&item).then_some(item))
    }

    fn find_after(
        pattern: &IUIAutomationItemContainerPattern,
        after: Option<&IUIAutomationElement>,
        name: &Variant,
    ) -> Option<IUIAutomationElement> {
        unsafe { pattern.FindItemByProperty(after, UIA_NamePropertyId, name.as_ref()) }.ok()
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, Deadline, UIAElement};

    pub(crate) fn realize_named_item(
        _container: &UIAElement,
        _value: &str,
        _deadline: Deadline,
    ) -> Result<Option<UIAElement>, AdapterError> {
        Ok(None)
    }
}

pub(crate) use imp::realize_named_item;

#[cfg(test)]
mod tests {
    use super::container_route_applies;

    #[test]
    fn the_container_route_runs_only_headed_and_only_when_the_walk_found_nothing() {
        assert!(container_route_applies(false, true));
        assert!(!container_route_applies(true, true));
        assert!(!container_route_applies(false, false));
        assert!(!container_route_applies(true, false));
    }
}
