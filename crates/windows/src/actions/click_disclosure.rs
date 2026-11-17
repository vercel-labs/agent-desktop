//! Click on a button whose only declared behavior is ExpandCollapse.
//!
//! Such a button has no Invoke pattern, so the generic click chain falls to
//! `LegacyIAccessible.DoDefaultAction`. Chromium accepts that call for a
//! popover invoker or a `<summary>` and changes nothing, which leaves the
//! caller with an unverified click that had no effect. Toggling the declared
//! ExpandCollapse state is the click's meaning for these buttons, and the
//! disclosure chain verifies the new state.

use crate::actions::disclosure::ExpandKind;

/// Route a click takes for one element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClickRoute {
    Chain,
    Disclose { want_expanded: bool },
}

/// Facts read from the element before the click is routed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClickTraits {
    pub(crate) invoke_available: bool,
    pub(crate) is_button: bool,
    pub(crate) expand_state: Option<ExpandKind>,
}

/// Toggle through ExpandCollapse only for a button with no Invoke and a
/// known expanded or collapsed state; everything else keeps the click chain.
pub(crate) fn click_route(traits: ClickTraits) -> ClickRoute {
    if traits.invoke_available || !traits.is_button {
        return ClickRoute::Chain;
    }
    match traits.expand_state {
        Some(ExpandKind::Collapsed) => ClickRoute::Disclose {
            want_expanded: true,
        },
        Some(ExpandKind::Expanded) => ClickRoute::Disclose {
            want_expanded: false,
        },
        _ => ClickRoute::Chain,
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{ClickRoute, ClickTraits, click_route};
    use crate::actions::disclosure::expand_state_if_available;
    use crate::tree::element::UIAElement;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use uiautomation::types::ControlType;

    /// Routes a click from the Invoke availability the caller already read,
    /// so a button with Invoke costs no extra property read.
    pub(crate) fn route_for(element: &UIAElement, invoke: bool) -> ClickRoute {
        if invoke {
            return ClickRoute::Chain;
        }
        let is_button = read_one(element, TreeProperty::ControlType).number()
            == Some(ControlType::Button as i32);
        if !is_button {
            return ClickRoute::Chain;
        }
        click_route(ClickTraits {
            invoke_available: invoke,
            is_button,
            expand_state: expand_state_if_available(element),
        })
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::route_for;

#[cfg(test)]
#[path = "click_disclosure_tests.rs"]
mod tests;
