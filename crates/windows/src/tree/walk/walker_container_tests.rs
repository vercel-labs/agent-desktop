use crate::tree::walk::walker_fake::{FakeTree, budget, walk};

const DATA_ITEM: i32 = 50029;
const TEXT: i32 = 50020;

fn walked_cell_actions(fake: &FakeTree) -> Vec<String> {
    let root = walk(fake, budget(10))
        .tree
        .into_accessibility_tree()
        .expect("a complete walk projects");
    let mut node = &root;
    while let Some(child) = node.children.first() {
        node = child;
    }
    node.presentation.available_actions.clone()
}

fn grid_cell_reads(
    control_type: i32,
) -> Vec<(
    crate::tree::properties::property_ids::TreeProperty,
    crate::tree::properties::property_outcome::PropertyOutcome,
)> {
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::test_support::{flag, number};
    vec![
        number(TreeProperty::ControlType, control_type),
        flag(TreeProperty::GridItemAvailable, true),
        flag(TreeProperty::InvokeAvailable, false),
    ]
}

/// A snapshot reads the same rule as a live read: a grid cell with no click of
/// its own advertises Click when a selectable row inside a grid holds it.
#[test]
fn a_snapshot_cell_in_a_selectable_row_of_a_grid_advertises_click() {
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::test_support::flag;
    let fake = FakeTree::default()
        .with_chain(&[1, 2, 3])
        .reading(1, &[flag(TreeProperty::GridAvailable, true)])
        .reading(2, &[flag(TreeProperty::SelectionItemAvailable, true)])
        .reading(3, &grid_cell_reads(DATA_ITEM));
    assert!(walked_cell_actions(&fake).contains(&"Click".to_string()));
}

#[test]
fn a_snapshot_cell_without_a_grid_above_its_row_does_not() {
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::test_support::flag;
    let fake = FakeTree::default()
        .with_chain(&[1, 2, 3])
        .reading(2, &[flag(TreeProperty::SelectionItemAvailable, true)])
        .reading(3, &grid_cell_reads(DATA_ITEM));
    assert!(!walked_cell_actions(&fake).contains(&"Click".to_string()));
}

/// Click must never create a ref: a text cell whose role is not interactive
/// keeps the actions it had, even inside a selectable row of a grid.
#[test]
fn a_snapshot_text_cell_with_a_non_interactive_role_gets_no_click() {
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::test_support::flag;
    let fake = FakeTree::default()
        .with_chain(&[1, 2, 3])
        .reading(1, &[flag(TreeProperty::GridAvailable, true)])
        .reading(2, &[flag(TreeProperty::SelectionItemAvailable, true)])
        .reading(3, &grid_cell_reads(TEXT));
    assert!(!walked_cell_actions(&fake).contains(&"Click".to_string()));
}
