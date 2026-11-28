//! Bounded SelectionItem descendant search and scroll steps for realizing items (A19-7).

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, normalize_search_text};

use crate::tree::element::UIAElement;

pub(crate) const MAX_SELECT_NODES: usize = 2_048;
pub(crate) const MAX_SELECT_DEPTH: u8 = 8;

fn select_node_budget_has_room(visited: usize, pending: usize, staged: usize) -> bool {
    visited.saturating_add(pending).saturating_add(staged) < MAX_SELECT_NODES
}

fn names_equal(left: &str, right: &str) -> bool {
    normalize_search_text(left) == normalize_search_text(right)
}

/// Second same-name SelectionItem is `AMBIGUOUS_TARGET`, never a silent pick.
pub(crate) fn accept_selection_match(already_found: bool) -> Result<(), AdapterError> {
    if already_found {
        Err(AdapterError::ambiguous_target(
            "Multiple SelectionItem elements share the requested accessible name",
        )
        .with_details(serde_json::json!({
            "kind": "ambiguous_select_value",
        })))
    } else {
        Ok(())
    }
}

/// Admits one live match into `found`, the match this scan observed. A match
/// that is not the element an earlier scan matched (`prior`), or not the one
/// this scan already holds, is a second item: `AMBIGUOUS_TARGET`. `prior` is
/// only compared against, so an element kept from before a scroll is never
/// the answer of a scan that did not see it.
pub(crate) fn admit_selection_match<T: Clone>(
    prior: Option<&T>,
    found: &mut Option<T>,
    candidate: &T,
    same: impl Fn(&T, &T) -> bool,
) -> Result<(), AdapterError> {
    let differs = |known: &T| !same(known, candidate);
    accept_selection_match(prior.is_some_and(differs) || found.as_ref().is_some_and(differs))?;
    found.get_or_insert_with(|| candidate.clone());
    Ok(())
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        AdapterError, Deadline, ErrorCode, MAX_SELECT_DEPTH, MAX_SELECT_NODES, UIAElement,
        admit_selection_match, names_equal, select_node_budget_has_room,
    };
    use crate::actions::mutation::{classify_success, classify_write};
    use crate::actions::scroll::SCROLL_LABEL;
    use crate::actions::select_locate::RealizeScrollStep;
    use crate::tree::automation::automation_client;
    use crate::tree::automation::uia_failure_error;
    use crate::tree::name_evidence::{name_fields, read_label};
    use crate::tree::properties::element_properties::ElementProperties;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use crate::tree::walk::walker::TreeSource;
    use crate::tree::walk::walker_source::{UiaTreeSource, same_element};
    use agent_desktop_core::LocatorField;
    use uiautomation::patterns::UIScrollPattern;
    use uiautomation::types::ScrollAmount;
    use windows::Win32::UI::Accessibility::UIA_ScrollPatternNoScroll;

    pub(crate) fn find_named_selection_item(
        root: &UIAElement,
        value: &str,
        deadline: Deadline,
        prior: Option<&UIAElement>,
    ) -> Result<Option<UIAElement>, AdapterError> {
        let client = automation_client()?;
        let source = UiaTreeSource::for_root(root)?;
        let prepared = source.prepare_root(root)?;
        let mut stack = Vec::new();
        push_children(&source, (&prepared, 1), deadline, 0, &mut stack)?;
        let mut visited = 0_usize;
        let mut found = None;
        while let Some((candidate, depth)) = stack.pop() {
            deadline.ensure_remaining()?;
            visited = visited.saturating_add(1);
            if visited > MAX_SELECT_NODES {
                return Err(node_budget_error());
            }
            if selection_item_available(&candidate) && name_matches(&candidate, value) {
                admit_selection_match(prior, &mut found, &candidate, |left, right| {
                    same_element(&client, left, right)
                })?;
            }
            if depth < MAX_SELECT_DEPTH {
                push_children(
                    &source,
                    (&candidate, depth.saturating_add(1)),
                    deadline,
                    visited,
                    &mut stack,
                )?;
            }
        }
        Ok(found)
    }

    /// Scrolls the container one small step down and reports whether its position stopped changing.
    pub(crate) fn realize_scroll_step(
        pattern: &UIScrollPattern,
        deadline: Deadline,
    ) -> Result<RealizeScrollStep, AdapterError> {
        deadline.ensure_remaining()?;
        let before = pattern.get_vertical_scroll_percent().ok();
        match pattern.scroll(ScrollAmount::NoAmount, ScrollAmount::SmallIncrement) {
            Ok(()) => {
                let _ = classify_success()?;
            }
            Err(error) => {
                let _ = classify_write("Scroll", SCROLL_LABEL, &error)?;
                return Ok(RealizeScrollStep::Stop);
            }
        }
        let after = pattern.get_vertical_scroll_percent().ok();
        Ok(RealizeScrollStep::Window {
            end_of_range: before.is_some() && before == after,
        })
    }

    /// Scrolls the container to a vertical position it reported earlier; `false` when it has no way to.
    pub(crate) fn scroll_to_position(
        pattern: &UIScrollPattern,
        position: f64,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        deadline.ensure_remaining()?;
        match pattern.set_scroll_percent(UIA_ScrollPatternNoScroll, position) {
            Ok(()) => classify_success(),
            Err(error) => classify_write("Scroll", SCROLL_LABEL, &error),
        }
    }

    pub(crate) fn selection_item_available(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::SelectionItemAvailable).flag() == Some(true)
    }

    pub(crate) fn name_matches(element: &UIAElement, value: &str) -> bool {
        let properties = ElementProperties::from_reads(vec![
            (TreeProperty::Name, read_one(element, TreeProperty::Name)),
            (
                TreeProperty::FullDescription,
                read_one(element, TreeProperty::FullDescription),
            ),
            (
                TreeProperty::HelpText,
                read_one(element, TreeProperty::HelpText),
            ),
            (
                TreeProperty::IsPassword,
                read_one(element, TreeProperty::IsPassword),
            ),
        ]);
        let label = read_label(element, false);
        let (name, _) = name_fields(&properties, &label);
        matches!(
            name,
            LocatorField::Known(text) if names_equal(&text, value)
        )
    }

    fn push_children(
        source: &UiaTreeSource,
        (parent, child_depth): (&UIAElement, u8),
        deadline: Deadline,
        visited: usize,
        stack: &mut Vec<(UIAElement, u8)>,
    ) -> Result<(), AdapterError> {
        deadline.ensure_remaining()?;
        let mut child = match source.first_child(parent) {
            Ok(child) => Some(child),
            Err(failure) if failure.is_exhaustion() => None,
            Err(failure) => {
                return Err(uia_failure_error(failure, "enumerate children for select"));
            }
        };
        let mut siblings = Vec::new();
        while let Some(current) = child {
            if !select_node_budget_has_room(visited, stack.len(), siblings.len()) {
                return Err(node_budget_error());
            }
            deadline.ensure_remaining()?;
            let next = match source.next_sibling(&current) {
                Ok(next) => Some(next),
                Err(failure) if failure.is_exhaustion() => None,
                Err(failure) => {
                    return Err(uia_failure_error(failure, "enumerate siblings for select"));
                }
            };
            siblings.push(current);
            child = next;
        }
        for sibling in siblings.into_iter().rev() {
            stack.push((sibling, child_depth));
        }
        Ok(())
    }

    fn node_budget_error() -> AdapterError {
        AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Select search exceeded its accessibility-node budget",
        )
        .with_details(serde_json::json!({
            "kind": "select_node_limit",
            "limit": MAX_SELECT_NODES,
            "complete": false,
        }))
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, Deadline, UIAElement};

    pub(crate) fn find_named_selection_item(
        _root: &UIAElement,
        _value: &str,
        _deadline: Deadline,
        _prior: Option<&UIAElement>,
    ) -> Result<Option<UIAElement>, AdapterError> {
        Ok(None)
    }

    pub(crate) fn selection_item_available(_element: &UIAElement) -> bool {
        false
    }

    pub(crate) fn name_matches(_element: &UIAElement, _value: &str) -> bool {
        false
    }
}

pub(crate) use imp::{find_named_selection_item, name_matches, selection_item_available};

#[cfg(target_os = "windows")]
pub(crate) use imp::{realize_scroll_step, scroll_to_position};

#[cfg(test)]
mod tests {
    use super::{
        MAX_SELECT_DEPTH, MAX_SELECT_NODES, accept_selection_match, names_equal,
        select_node_budget_has_room,
    };
    use agent_desktop_core::ErrorCode;

    #[test]
    fn names_compare_by_the_one_core_name_form() {
        assert!(names_equal("Éa", "éa"));
        assert!(names_equal("  a \t b ", "A b"));
        assert!(!names_equal("a", "b"));
        assert!(!names_equal("a b", "ab"));
    }

    #[test]
    fn select_search_depth_budget_is_eight() {
        assert_eq!(MAX_SELECT_DEPTH, 8);
    }

    #[test]
    fn sibling_staging_reserves_space_with_visited_and_pending_nodes() {
        assert!(select_node_budget_has_room(MAX_SELECT_NODES - 1, 0, 0));
        assert!(!select_node_budget_has_room(MAX_SELECT_NODES - 1, 1, 0));
        assert!(!select_node_budget_has_room(1_000, 1_000, 48));
    }

    #[test]
    fn first_selection_match_is_accepted() {
        accept_selection_match(false).expect("first");
    }

    #[test]
    fn second_selection_match_is_ambiguous() {
        let error = accept_selection_match(true).expect_err("duplicate");
        assert_eq!(error.code, ErrorCode::AmbiguousTarget);
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("kind"))
                .and_then(serde_json::Value::as_str),
            Some("ambiguous_select_value")
        );
    }
}

#[cfg(test)]
#[path = "select_search_match_tests.rs"]
mod match_tests;
