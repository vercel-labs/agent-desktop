use super::collapse_grids;
use crate::AccessibilityNode;

fn node(role: &str, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: Default::default(),
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

fn grid(rows: u32, columns: u32, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut table = node("table", children);
    table.presentation.descriptors.row_count = Some(rows);
    table.presentation.descriptors.column_count = Some(columns);
    table
}

fn cells(count: usize) -> Vec<AccessibilityNode> {
    (0..count).map(|_| node("cell", Vec::new())).collect()
}

#[test]
fn a_grid_below_the_root_keeps_its_size_and_loses_its_cells() {
    let mut window = node("window", vec![node("group", vec![grid(2, 3, cells(6))])]);

    collapse_grids(&mut window);

    let table = &window.children[0].children[0];
    assert!(table.children.is_empty());
    assert_eq!(table.children_count, Some(6));
    assert_eq!(table.presentation.descriptors.row_count, Some(2));
    assert_eq!(table.presentation.descriptors.column_count, Some(3));
}

#[test]
fn the_root_of_a_drill_keeps_its_cells() {
    let mut table = grid(2, 3, cells(6));

    collapse_grids(&mut table);

    assert_eq!(table.children.len(), 6);
    assert_eq!(table.children_count, None);
}

#[test]
fn a_container_without_a_grid_size_is_left_alone() {
    let mut window = node("window", vec![node("list", cells(4))]);

    collapse_grids(&mut window);

    assert_eq!(window.children[0].children.len(), 4);
    assert_eq!(window.children[0].children_count, None);
}

#[test]
fn a_count_the_walk_already_recorded_is_kept() {
    let mut table = grid(2, 3, cells(2));
    table.children_count = Some(9);
    let mut window = node("window", vec![table]);

    collapse_grids(&mut window);

    assert_eq!(window.children[0].children_count, Some(9));
}
