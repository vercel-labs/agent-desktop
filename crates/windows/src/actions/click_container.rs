//! Selecting a cell through the row or list item that holds it.
//!
//! A text cell of a grid or list has no action of its own, while the item that
//! contains it advertises `SelectionItemPattern`. Clicking such a cell selects
//! that item. Only an item that sits inside a grid or list qualifies, so the
//! climb never selects an unrelated ancestor.

/// What one ancestor of the clicked element offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AncestorFacts {
    pub selection_item: bool,
    pub container: bool,
}

impl AncestorFacts {
    /// What an element offers as an ancestor, from the properties a walk
    /// already read.
    pub(crate) fn of(properties: &crate::tree::properties::ElementProperties) -> Self {
        use crate::tree::properties::property_ids::TreeProperty;
        Self {
            selection_item: properties.is_true(TreeProperty::SelectionItemAvailable),
            container: properties.is_true(TreeProperty::GridAvailable)
                || properties.is_true(TreeProperty::SelectionAvailable),
        }
    }
}

/// How far above the element the climb looks.
pub(crate) const MAX_CLIMB: usize = 6;

/// The nearest ancestor that is a selection item with a grid or list above
/// it, as an index into `ancestors` (the parent first).
pub(crate) fn container_item_index(ancestors: &[AncestorFacts]) -> Option<usize> {
    let item = ancestors.iter().position(|facts| facts.selection_item)?;
    ancestors[item + 1..]
        .iter()
        .any(|facts| facts.container)
        .then_some(item)
}

/// The action list of a grid cell, with `Click` added when the cell has no
/// click of its own and the item holding it can be selected.
///
/// The container is looked up only for a grid cell that does not already
/// advertise `Click`, so no other element pays for the climb.
pub(crate) fn with_container_click(
    mut actions: Vec<String>,
    grid_cell: bool,
    has_container_item: impl FnOnce() -> bool,
) -> Vec<String> {
    let click = agent_desktop_core::actions::capability::CLICK;
    if grid_cell && !actions.iter().any(|action| action == click) && has_container_item() {
        actions.push(click.to_string());
    }
    actions
}

/// Whether an element may be offered the container click: a grid cell whose
/// role already earns it a ref. Click must never create a ref, so a text cell
/// with a non-interactive role is left as it was.
pub(crate) fn is_click_candidate(
    grid_cell: bool,
    role: &agent_desktop_core::LocatorField<String>,
) -> bool {
    grid_cell
        && matches!(
            role,
            agent_desktop_core::LocatorField::Known(role)
                if agent_desktop_core::tree::roles::is_interactive_role(role)
        )
}

/// `with_container_click` over a known action list; an unknown list stays
/// unknown.
pub(crate) fn extend_known_actions(
    actions: agent_desktop_core::LocatorField<Vec<String>>,
    grid_cell: bool,
    has_container_item: impl FnOnce() -> bool,
) -> agent_desktop_core::LocatorField<Vec<String>> {
    match actions {
        agent_desktop_core::LocatorField::Known(list) => agent_desktop_core::LocatorField::Known(
            with_container_click(list, grid_cell, has_container_item),
        ),
        other => other,
    }
}

/// Whether the ancestors a walk has passed (the parent last) hold a selection
/// item inside a grid or list, looking at most `MAX_CLIMB` levels up.
pub(crate) fn walk_has_container_item(passed: &[AncestorFacts]) -> bool {
    let nearest_first: Vec<AncestorFacts> = passed.iter().rev().take(MAX_CLIMB).copied().collect();
    container_item_index(&nearest_first).is_some()
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{AncestorFacts, MAX_CLIMB, container_item_index};
    use crate::tree::automation::automation_client;
    use crate::tree::element::UIAElement;
    use crate::tree::properties::ElementProperties;
    use crate::tree::properties::element_properties::known_as_bool;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::property_outcome::PropertyOutcome;
    use crate::tree::properties::{read_live_bounded, read_one};
    use crate::tree::walk::walker::walk_role;
    use crate::tree::walk::walker_source::parent_step;
    use agent_desktop_core::{Deadline, LocatorField};

    /// The selection item that holds `element` inside a grid or list, if any.
    fn container_item(element: &UIAElement) -> Option<UIAElement> {
        let walker = automation_client().ok()?.get_raw_view_walker().ok()?;
        let mut ancestors = Vec::new();
        let mut facts = Vec::new();
        let mut current = element.clone();
        for _ in 0..MAX_CLIMB {
            let Ok(Some(parent)) = parent_step(&walker, &current) else {
                break;
            };
            facts.push(AncestorFacts {
                selection_item: is_true(&parent, TreeProperty::SelectionItemAvailable),
                container: is_true(&parent, TreeProperty::GridAvailable)
                    || is_true(&parent, TreeProperty::SelectionAvailable),
            });
            ancestors.push(parent.clone());
            current = parent;
        }
        let index = container_item_index(&facts)?;
        ancestors.into_iter().nth(index)
    }

    /// The item a click on `element` selects: the one the snapshot advertises
    /// `Click` for, so only a grid cell with an interactive role has one. Any
    /// other element costs one property read and is never climbed from.
    pub(crate) fn clicked_item(element: &UIAElement, deadline: Deadline) -> Option<UIAElement> {
        if !is_true(element, TreeProperty::GridItemAvailable) {
            return None;
        }
        let (properties, _) = read_live_bounded(element, deadline);
        super::is_click_candidate(true, &walk_role(&properties))
            .then(|| container_item(element))
            .flatten()
    }

    /// The live action list, extended for a grid cell the way a click can
    /// select it through its row.
    pub(crate) fn advertise_container_click(
        element: &UIAElement,
        properties: &ElementProperties,
        role: &LocatorField<String>,
        actions: LocatorField<Vec<String>>,
    ) -> LocatorField<Vec<String>> {
        super::extend_known_actions(
            actions,
            super::is_click_candidate(properties.is_true(TreeProperty::GridItemAvailable), role),
            || container_item(element).is_some(),
        )
    }

    fn is_true(element: &UIAElement, property: TreeProperty) -> bool {
        matches!(
            read_one(element, property),
            PropertyOutcome::Known(value) if known_as_bool(&value) == Some(true)
        )
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::{advertise_container_click, clicked_item};

#[cfg(test)]
#[path = "click_container_tests.rs"]
mod tests;
