use super::*;
use crate::tree::app_menu::find_menu_descendant_with;
use agent_desktop_core::ErrorCode;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Default)]
struct FakeTree {
    roles: HashMap<u8, &'static str>,
    subroles: HashMap<u8, &'static str>,
    hidden: Vec<u8>,
    children: HashMap<u8, Vec<u8>>,
    failing_reads: Vec<u8>,
    role_reads: usize,
    subrole_reads: usize,
}

impl FakeTree {
    fn node(mut self, id: u8, role: &'static str, children: &[u8]) -> Self {
        self.roles.insert(id, role);
        self.children.insert(id, children.to_vec());
        self
    }

    fn subrole(mut self, id: u8, subrole: &'static str) -> Self {
        self.subroles.insert(id, subrole);
        self
    }

    fn hidden(mut self, id: u8) -> Self {
        self.hidden.push(id);
        self
    }
}

impl MenuTreeReader<u8> for FakeTree {
    fn string(&mut self, element: &u8, attribute: &str) -> Result<Option<String>, AdapterError> {
        if self.failing_reads.contains(element) {
            return Err(AdapterError::new(ErrorCode::PermDenied, "denied"));
        }
        let table = if attribute == "AXRole" {
            self.role_reads += 1;
            &self.roles
        } else {
            self.subrole_reads += 1;
            &self.subroles
        };
        Ok(table.get(element).map(|value| (*value).to_string()))
    }

    fn boolean(&mut self, element: &u8, _attribute: &str) -> Result<Option<bool>, AdapterError> {
        Ok(self.hidden.contains(element).then_some(false))
    }

    fn children(&mut self, element: &u8) -> Result<Vec<u8>, AdapterError> {
        Ok(self.children.get(element).cloned().unwrap_or_default())
    }
}

fn search(tree: &mut FakeTree) -> (Option<(String, u8)>, PopupContainer<u8>) {
    let mut container = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    let found = find_menu_descendant_with(
        Lineage::root(0_u8, "app".into()),
        "app".into(),
        deadline,
        |node| inspect_node(node, tree, Some(&mut container)),
    )
    .unwrap();
    (found.map(|(path, node)| (path, node.element)), container)
}

fn popup_in_dialog() -> FakeTree {
    FakeTree::default()
        .node(0, "AXWindow", &[1, 2])
        .node(1, "AXGroup", &[])
        .node(2, "AXGroup", &[3])
        .node(3, "AXGroup", &[4, 5, 6])
        .node(4, "AXMenuItem", &[])
        .node(5, "AXMenuItem", &[])
        .node(6, "AXMenuItem", &[])
}

#[test]
fn menu_items_without_a_menu_ancestor_report_their_container() {
    let (menu, container) = search(&mut popup_in_dialog());
    assert_eq!(menu, None);
    assert_eq!(container, Some(("app/children/1/children/0".into(), 3)));
}

#[test]
fn a_real_menu_wins_over_a_shallower_menu_item_container() {
    let mut tree = popup_in_dialog().node(1, "AXMenu", &[]);
    let (menu, container) = search(&mut tree);
    assert_eq!(menu, Some(("app/children/0".into(), 1)));
    assert_eq!(container, None);
}

#[test]
fn menu_items_inside_a_landmark_region_are_page_chrome() {
    for landmark in ["AXLandmarkNavigation", "AXLandmarkComplementary"] {
        let mut tree = popup_in_dialog().subrole(2, landmark);
        assert_eq!(search(&mut tree).1, None, "{landmark}");
    }
}

#[test]
fn a_landmark_beside_the_container_does_not_disqualify_it() {
    let mut tree = popup_in_dialog().subrole(1, "AXLandmarkNavigation");
    assert_eq!(
        search(&mut tree).1,
        Some(("app/children/1/children/0".into(), 3))
    );
}

#[test]
fn hidden_containers_are_not_open_menus() {
    let mut tree = popup_in_dialog().hidden(3);
    assert_eq!(search(&mut tree).1, None);
}

#[test]
fn hidden_menu_items_are_not_evidence_of_an_open_menu() {
    let mut tree = popup_in_dialog().hidden(4).hidden(5).hidden(6);
    assert_eq!(search(&mut tree).1, None);
}

#[test]
fn a_hidden_item_does_not_mask_a_later_visible_popup() {
    let mut tree = popup_in_dialog()
        .node(0, "AXWindow", &[1, 2, 7])
        .node(7, "AXGroup", &[8])
        .node(8, "AXMenuItem", &[])
        .hidden(4)
        .hidden(5)
        .hidden(6);
    assert_eq!(search(&mut tree).1, Some(("app/children/2".into(), 7)));
}

#[test]
fn a_hidden_ancestor_above_the_container_hides_the_menu() {
    let mut tree = popup_in_dialog().hidden(2);
    assert_eq!(search(&mut tree).1, None);
}

#[test]
fn a_hidden_branch_does_not_mask_a_visible_popup_beside_it() {
    let mut tree = popup_in_dialog()
        .node(0, "AXWindow", &[1, 2, 7])
        .node(7, "AXGroup", &[8])
        .node(8, "AXMenuItem", &[])
        .hidden(2);
    assert_eq!(search(&mut tree).1, Some(("app/children/2".into(), 7)));
}

#[test]
fn menu_roles_never_become_popup_containers() {
    let mut hidden_menu = popup_in_dialog().node(3, "AXMenu", &[4, 5, 6]).hidden(3);
    assert_eq!(search(&mut hidden_menu).1, None);
    let mut bar_item = popup_in_dialog().node(3, "AXMenuBarItem", &[4, 5, 6]);
    assert_eq!(search(&mut bar_item).1, None);
}

#[test]
fn items_nested_in_an_item_are_reported_through_the_item_that_holds_them() {
    let mut tree = popup_in_dialog().node(3, "AXMenuItem", &[4, 5, 6]);
    assert_eq!(search(&mut tree).1, Some(("app/children/1".into(), 2)));
}

#[test]
fn menu_bars_are_not_searched() {
    let mut tree = FakeTree::default()
        .node(0, "AXApplication", &[1])
        .node(1, "AXMenuBar", &[2])
        .node(2, "AXMenuItem", &[]);
    let (menu, container) = search(&mut tree);
    assert_eq!((menu, container), (None, None));
}

#[test]
fn a_menu_item_without_a_parent_reports_nothing() {
    let mut tree = FakeTree::default().node(0, "AXMenuItem", &[]);
    assert_eq!(search(&mut tree), (None, None));
}

#[test]
fn sibling_items_share_one_ancestor_walk() {
    let mut tree = popup_in_dialog();
    let (_, container) = search(&mut tree);
    assert_eq!(container.map(|(_, element)| element), Some(3));
    assert_eq!(tree.subrole_reads, 3);
}

#[test]
fn the_shallowest_container_wins() {
    let mut tree = popup_in_dialog()
        .node(1, "AXGroup", &[7])
        .node(7, "AXGroup", &[8])
        .node(8, "AXMenuItem", &[]);
    let (_, container) = search(&mut tree);
    assert_eq!(container, Some(("app/children/0/children/0".into(), 7)));
}

#[test]
fn without_a_container_slot_no_ancestor_attribute_is_read() {
    let mut tree = popup_in_dialog();
    let deadline = Instant::now() + Duration::from_secs(5);
    find_menu_descendant_with(
        Lineage::root(0_u8, "app".into()),
        "app".into(),
        deadline,
        |node| inspect_node(node, &mut tree, None),
    )
    .unwrap();
    assert_eq!(tree.subrole_reads, 0);
    assert_eq!(tree.role_reads, tree.roles.len());
}

#[test]
fn ancestor_read_failures_propagate_instead_of_guessing() {
    let mut tree = popup_in_dialog();
    tree.failing_reads.push(2);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut container = None;
    let error = find_menu_descendant_with(
        Lineage::root(0_u8, "app".into()),
        "app".into(),
        deadline,
        |node| inspect_node(node, &mut tree, Some(&mut container)),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::PermDenied);
}
