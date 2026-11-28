//! Select.
//!
//! A native combo box that stores item names selects by name through its own
//! window messages (`select_combo`), and a name its search misses is not
//! found; an owner-drawn one falls through to the route below. A tab item of a native tab
//! control selects through `TCM_SETCURFOCUS` (`select_tab`), because the
//! accessibility select route changes the index without the notification
//! that swaps the page. Any other target selects a named item, itself or one of
//! its realized descendants: LegacyIAccessible `Select(SELFLAG_TAKESELECTION)`
//! has no foreground effect on SysTreeView32, WinForms, WPF and DirectUI items
//! (C8), SelectionItemPattern takes none either (A31-5), and the item's
//! IsSelected verifies either one. A target that is itself the named item selects
//! itself before anything else, so a collapsed tree item never expands. A collapsed container that is not a native combo box
//! (a WPF or Ribbon combo) shows its items only after it expands, and every
//! route there puts the popup over the user's window (C5, C6): the expand runs
//! through the core chain, so a policy that does not allow focus steal is
//! refused there, and a policy that does expands it first and collapses it
//! again on failure; the error reports the collapse as the state read proved
//! it, and an unproven collapse marks the delivered expand unsafe to retry.

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliveryOutcome, ErrorCode, InteractionPolicy,
};
use std::time::Duration;

use crate::actions::chain::{after_delivery, offered, poll_until, rung};
use crate::actions::disclosure::{EXPAND_LABEL, ExpandKind};
use crate::tree::element::UIAElement;

pub(crate) const SELECT_LABEL: &str = crate::actions::chain::SELECTION_ITEM_LABEL;

const VERIFY_TIMEOUT: Duration = Duration::from_millis(600);

const SELECT_SUGGESTION: &str = "Use find or snapshot to inspect the available selection items.";

fn run_expand(
    expand: &mut dyn FnMut() -> Result<DeliveryOutcome, AdapterError>,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    agent_desktop_core::execute_chain(
        &mut offered([rung(EXPAND_LABEL, true, true, expand)]),
        policy,
        deadline,
        SELECT_SUGGESTION,
    )
    .map(|_| ())
}

/// Applies what the container's collapse read proved to the select error.
/// `container_collapsed` is `true` only when the read reported `Collapsed`,
/// `false` when it reported any other state, and `null` when no read
/// answered. Unless the read proved the container collapsed again, the
/// expand that already ran is a delivered mutation, so the error carries
/// `delivered_unverified` and an unsafe retry whatever the select itself
/// reported.
fn annotate_container(error: AdapterError, collapsed: Option<bool>) -> AdapterError {
    let error = match collapsed {
        Some(true) => error,
        _ => after_delivery(error),
    };
    error.with_details(serde_json::json!({
        "container_expanded": true,
        "container_collapsed": collapsed,
    }))
}

/// Runs the best-effort `collapse` write, then polls `read` — the
/// container's `ExpandCollapseState` — until it reports `Collapsed` or the
/// verify budget capped by `deadline` ends. The write's acknowledgement is
/// not the report: only the read decides, so an acked collapse that left the
/// container open reports `Some(false)`, and a read that never answers
/// reports `None`.
fn collapse_container(
    collapse: &mut dyn FnMut() -> bool,
    read: &mut dyn FnMut() -> Option<ExpandKind>,
    deadline: Deadline,
) -> Option<bool> {
    let _ = collapse();
    let mut last = None;
    let _ = poll_until(deadline, VERIFY_TIMEOUT, || {
        last = read();
        matches!(last, Some(ExpandKind::Collapsed))
    });
    last.map(|state| state == ExpandKind::Collapsed)
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        ActionStep, AdapterError, Deadline, DeliveryOutcome, EXPAND_LABEL, ErrorCode,
        InteractionPolicy, SELECT_LABEL, SELECT_SUGGESTION, UIAElement, VERIFY_TIMEOUT,
        annotate_container, collapse_container, offered, run_expand, rung,
    };
    use crate::actions::chain::{after_delivery, legacy_select, poll_until, selection_item_select};
    use crate::actions::click::SELECT_ITEM_LABEL;
    use crate::actions::disclosure::ExpandKind;
    use crate::actions::mutation::{classify_success, classify_write};
    use crate::actions::scroll::vertical_scroll;
    use crate::actions::select_combo::{COMBO_SELECT_LABEL, native_combo, select_native_combo};
    use crate::actions::select_container::{Realization, realize_named_item};
    use crate::actions::select_locate::{ItemSource, Located, RealizeScrollStep, locate};
    use crate::actions::select_search::{
        find_named_selection_item, name_matches, realize_scroll_step, scroll_to_position,
        selection_item_available,
    };
    use crate::actions::select_tab::{TAB_SELECT_LABEL, select_native_tab};
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use agent_desktop_core::DeliverySemantics;
    use uiautomation::patterns::{UIExpandCollapsePattern, UIScrollPattern};

    pub(crate) fn select_steps(
        element: &UIAElement,
        value: &str,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        deadline.ensure_remaining()?;
        if let Some(combo) = native_combo(element) {
            if let Some(outcome) = select_native_combo(combo, value, deadline)? {
                return Ok(vec![outcome.step(COMBO_SELECT_LABEL)]);
            }
        }
        if name_matches(element, value)
            && let Some(outcome) = select_native_tab(element, deadline)?
        {
            return Ok(vec![outcome.step(TAB_SELECT_LABEL)]);
        }
        if selection_item_available(element) && name_matches(element, value) {
            return select_item(element, policy, deadline);
        }
        let collapsed = is_collapsed_container(element);
        if collapsed {
            let mut expand = || expand_container(element);
            run_expand(&mut expand, policy, deadline)?;
        }
        let selected = find_item(element, value, deadline).and_then(|(item, realized)| {
            select_item(&item, policy, deadline).map_err(|error| match realized {
                true => after_delivery(error),
                false => error,
            })
        });
        match selected {
            Err(error) if collapsed => {
                let mut collapse = || collapse_pattern(element);
                let mut read = || read_expand_state(element);
                Err(annotate_container(
                    error,
                    collapse_container(&mut collapse, &mut read, deadline),
                ))
            }
            other => other,
        }
    }

    /// The selection items of one container, read through UI Automation.
    struct ContainerItems<'a> {
        element: &'a UIAElement,
        value: &'a str,
        deadline: Deadline,
        scroll: Option<UIScrollPattern>,
    }

    impl<'a> ContainerItems<'a> {
        fn new(element: &'a UIAElement, value: &'a str, deadline: Deadline) -> Self {
            Self {
                element,
                value,
                deadline,
                scroll: vertical_scroll(element),
            }
        }
    }

    impl ItemSource for ContainerItems<'_> {
        type Item = UIAElement;

        fn scan(&mut self, prior: Option<&UIAElement>) -> Result<Option<UIAElement>, AdapterError> {
            find_named_selection_item(self.element, self.value, self.deadline, prior)
        }

        fn position(&self) -> Option<f64> {
            self.scroll
                .as_ref()
                .and_then(|pattern| pattern.get_vertical_scroll_percent().ok())
        }

        fn scroll_step(&mut self) -> Result<RealizeScrollStep, AdapterError> {
            match &self.scroll {
                Some(pattern) => realize_scroll_step(pattern, self.deadline),
                None => Ok(RealizeScrollStep::Stop),
            }
        }

        fn scroll_to(&mut self, position: f64) -> Result<bool, AdapterError> {
            match &self.scroll {
                Some(pattern) => scroll_to_position(pattern, position, self.deadline),
                None => Ok(false),
            }
        }

        fn realize(&mut self) -> Result<Option<(UIAElement, Realization)>, AdapterError> {
            realize_named_item(self.element, self.value, self.deadline)
        }
    }

    /// Locates the named selection item inside `element`, scrolling to
    /// realize it when the container hides items.
    fn find_item(
        element: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<(UIAElement, bool), AdapterError> {
        let mut items = ContainerItems::new(element, value, deadline);
        match locate(&mut items)? {
            Located::Found { item, realized } => Ok((item, realized)),
            Located::Absent => Err(AdapterError::new(
                ErrorCode::ElementNotFound,
                format!(
                    "No selection item matched the requested value ({} chars)",
                    value.chars().count()
                ),
            )
            .with_disposition(DeliverySemantics::not_delivered())
            .with_suggestion(SELECT_SUGGESTION)),
        }
    }

    fn select_item(
        item: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let observe = |delivered: bool| -> Result<DeliveryOutcome, AdapterError> {
            if !delivered {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            let selected = poll_until(deadline, VERIFY_TIMEOUT, || {
                read_one(item, TreeProperty::SelectionItemIsSelected).flag() == Some(true)
            });
            Ok(DeliveryOutcome::from_delivery(true, selected))
        };
        let mut legacy = || observe(legacy_select(item)?);
        let mut pattern = || observe(selection_item_select(item)?);
        agent_desktop_core::execute_chain(
            &mut offered([
                rung(
                    SELECT_LABEL,
                    selection_item_available(item),
                    false,
                    &mut pattern,
                ),
                rung(SELECT_ITEM_LABEL, true, false, &mut legacy),
            ]),
            policy,
            deadline,
            SELECT_SUGGESTION,
        )
    }

    fn expand_container(element: &UIAElement) -> Result<DeliveryOutcome, AdapterError> {
        let delivered = match element.0.get_pattern::<UIExpandCollapsePattern>() {
            Ok(pattern) => match pattern.expand() {
                Ok(()) => classify_success()?,
                Err(error) => classify_write("Expand", EXPAND_LABEL, &error)?,
            },
            Err(error) => classify_write("get_pattern", EXPAND_LABEL, &error)?,
        };
        Ok(DeliveryOutcome::from_delivery(delivered, false))
    }

    fn collapse_pattern(element: &UIAElement) -> bool {
        element
            .0
            .get_pattern::<UIExpandCollapsePattern>()
            .is_ok_and(|pattern| pattern.collapse().is_ok())
    }

    fn read_expand_state(element: &UIAElement) -> Option<ExpandKind> {
        if read_one(element, TreeProperty::ExpandCollapseAvailable).flag() != Some(true) {
            return None;
        }
        read_one(element, TreeProperty::ExpandCollapseState)
            .number()
            .and_then(ExpandKind::from_i32)
    }

    fn is_collapsed_container(element: &UIAElement) -> bool {
        read_expand_state(element) == Some(ExpandKind::Collapsed)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionStep, AdapterError, Deadline, InteractionPolicy, UIAElement};

    pub(crate) fn select_steps(
        _element: &UIAElement,
        _value: &str,
        _policy: InteractionPolicy,
        _deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Select"))
    }
}

pub(crate) use imp::select_steps;

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
