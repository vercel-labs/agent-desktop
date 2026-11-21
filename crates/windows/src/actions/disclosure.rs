//! Expand and Collapse.
//!
//! The ExpandCollapse state is read first, and a node already in the target
//! state delivers nothing. A tree item's LegacyIAccessible default action
//! flips it with no foreground change on SysTreeView32, WinForms and WPF
//! trees (C7), so it runs only from a known opposite state and is verified by
//! re-reading the state. ExpandCollapsePattern activates the window and is
//! the headed rung. On a combo box every route puts the popup over the user's
//! window (C6), so headless offers it no rung and refuses.

use agent_desktop_core::{ActionStep, AdapterError, Deadline, InteractionPolicy};
use std::time::Duration;

use crate::tree::element::UIAElement;

pub(crate) const EXPAND_LABEL: &str = "ExpandCollapsePattern.Expand";
pub(crate) const COLLAPSE_LABEL: &str = "ExpandCollapsePattern.Collapse";

const VERIFY_TIMEOUT: Duration = Duration::from_millis(600);

const DISCLOSURE_SUGGESTION: &str =
    "Refresh the snapshot and retry, or target an expandable container.";

/// UIA `ExpandCollapseState` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExpandKind {
    Collapsed,
    Expanded,
    PartiallyExpanded,
    LeafNode,
}

impl ExpandKind {
    pub(crate) fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Collapsed),
            1 => Some(Self::Expanded),
            2 => Some(Self::PartiallyExpanded),
            3 => Some(Self::LeafNode),
            _ => None,
        }
    }

    fn is_target(self, want_expanded: bool) -> bool {
        matches!(
            (self, want_expanded),
            (Self::Expanded, true) | (Self::Collapsed, false)
        )
    }

    fn is_opposite(self, want_expanded: bool) -> bool {
        matches!(
            (self, want_expanded),
            (Self::Collapsed, true) | (Self::Expanded, false)
        )
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        ActionStep, AdapterError, COLLAPSE_LABEL, DISCLOSURE_SUGGESTION, Deadline, EXPAND_LABEL,
        ExpandKind, InteractionPolicy, UIAElement, VERIFY_TIMEOUT,
    };
    use crate::actions::chain::{
        ALREADY_LABEL, LEGACY_LABEL, legacy_default_action, legacy_default_action_available,
        offered, poll_until, rung,
    };
    use crate::actions::mutation::{classify_success, classify_write};
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use agent_desktop_core::DeliveryOutcome;
    use uiautomation::patterns::UIExpandCollapsePattern;
    use uiautomation::types::ControlType;

    pub(crate) fn disclosure_steps(
        element: &UIAElement,
        want_expanded: bool,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        deadline.ensure_remaining()?;
        let pattern_ok =
            read_one(element, TreeProperty::ExpandCollapseAvailable).flag() == Some(true);
        let current = read_expand_state(element);
        if current.is_some_and(|state| state.is_target(want_expanded)) {
            return Ok(vec![DeliveryOutcome::Satisfied.step(ALREADY_LABEL)]);
        }
        let flips = current.is_some_and(|state| state.is_opposite(want_expanded))
            && read_one(element, TreeProperty::ControlType).number()
                == Some(ControlType::TreeItem as i32)
            && legacy_default_action_available(element);
        let label = if want_expanded {
            EXPAND_LABEL
        } else {
            COLLAPSE_LABEL
        };
        let mut legacy =
            || deliver_and_observe(element, want_expanded, deadline, legacy_default_action);
        let mut pattern = || {
            deliver_and_observe(element, want_expanded, deadline, |element| {
                pattern_expand_collapse(element, want_expanded, label)
            })
        };
        agent_desktop_core::execute_chain(
            &mut offered([
                rung(
                    label,
                    pattern_ok && current != Some(ExpandKind::LeafNode),
                    true,
                    &mut pattern,
                ),
                rung(LEGACY_LABEL, flips, false, &mut legacy),
            ]),
            policy,
            deadline,
            DISCLOSURE_SUGGESTION,
        )
    }

    fn deliver_and_observe(
        element: &UIAElement,
        want_expanded: bool,
        deadline: Deadline,
        deliver: impl FnOnce(&UIAElement) -> Result<bool, AdapterError>,
    ) -> Result<DeliveryOutcome, AdapterError> {
        if !deliver(element)? {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        let verified = poll_until(deadline, VERIFY_TIMEOUT, || {
            read_expand_state(element).is_some_and(|state| state.is_target(want_expanded))
        });
        Ok(DeliveryOutcome::from_delivery(true, verified))
    }

    fn read_expand_state(element: &UIAElement) -> Option<ExpandKind> {
        if read_one(element, TreeProperty::ExpandCollapseAvailable).flag() != Some(true) {
            return None;
        }
        read_one(element, TreeProperty::ExpandCollapseState)
            .number()
            .and_then(ExpandKind::from_i32)
    }

    fn pattern_expand_collapse(
        element: &UIAElement,
        want_expanded: bool,
        label: &'static str,
    ) -> Result<bool, AdapterError> {
        let operation = if want_expanded { "Expand" } else { "Collapse" };
        match element.0.get_pattern::<UIExpandCollapsePattern>() {
            Ok(pattern) => {
                let result = if want_expanded {
                    pattern.expand()
                } else {
                    pattern.collapse()
                };
                match result {
                    Ok(()) => classify_success(),
                    Err(error) => classify_write(operation, label, &error),
                }
            }
            Err(error) => classify_write("get_pattern", label, &error),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionStep, AdapterError, Deadline, InteractionPolicy, UIAElement};

    pub(crate) fn disclosure_steps(
        _: &UIAElement,
        _: bool,
        _: InteractionPolicy,
        _: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("ExpandCollapse"))
    }
}

pub(crate) use imp::disclosure_steps;
