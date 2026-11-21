//! Bounded SelectionItem descendant search (A19-7).

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use crate::tree::element::UIAElement;

pub(crate) const MAX_SELECT_NODES: usize = 2_048;
pub(crate) const MAX_SELECT_DEPTH: u8 = 8;

fn select_node_budget_has_room(visited: usize, pending: usize, staged: usize) -> bool {
    visited.saturating_add(pending).saturating_add(staged) < MAX_SELECT_NODES
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

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        AdapterError, Deadline, ErrorCode, MAX_SELECT_DEPTH, MAX_SELECT_NODES, UIAElement,
        accept_selection_match, select_node_budget_has_room,
    };
    use crate::tree::automation::automation_client;
    use crate::tree::automation::uia_failure_error;
    use crate::tree::name_evidence::{name_fields, read_label};
    use crate::tree::properties::element_properties::ElementProperties;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use crate::tree::walk::walker::TreeSource;
    use crate::tree::walk::walker_source::{UiaTreeSource, same_element};
    use agent_desktop_core::LocatorField;

    pub(crate) fn find_named_selection_item(
        root: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<Option<UIAElement>, AdapterError> {
        let client = automation_client()?;
        let source = UiaTreeSource::for_root(root)?;
        let prepared = source.prepare_root(root)?;
        let mut stack = Vec::new();
        push_children(&source, (&prepared, 1), deadline, 0, &mut stack)?;
        let mut visited = 0_usize;
        let mut found: Option<UIAElement> = None;
        while let Some((candidate, depth)) = stack.pop() {
            deadline.ensure_remaining()?;
            visited = visited.saturating_add(1);
            if visited > MAX_SELECT_NODES {
                return Err(node_budget_error());
            }
            if selection_item_available(&candidate) && name_matches(&candidate, value) {
                match &found {
                    None => found = Some(candidate.clone()),
                    Some(prev) if same_element(&client, prev, &candidate) => {}
                    Some(_) => {
                        accept_selection_match(true)?;
                    }
                }
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
            LocatorField::Known(text) if text.eq_ignore_ascii_case(value)
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

#[cfg(test)]
mod tests {
    use super::{
        MAX_SELECT_DEPTH, MAX_SELECT_NODES, accept_selection_match, select_node_budget_has_room,
    };
    use agent_desktop_core::ErrorCode;

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
