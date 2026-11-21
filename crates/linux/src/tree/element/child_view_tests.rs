use std::sync::atomic::Ordering;

use super::*;
use crate::tree::element::child_list::list_children;
use crate::tree::test_support::grid_fake::{BUS, TABLE_PATH, grid};

fn read(
    reported: i32,
    limit: usize,
    interfaces: Vec<String>,
    columns: i32,
    rows: i32,
) -> (
    ChildView,
    std::sync::Arc<crate::tree::test_support::grid_fake::Calls>,
) {
    let (client, element, calls, _server) = grid(reported, columns, rows);
    let view = zbus::block_on(read_child_view(
        &client,
        &element,
        &Field::Present(reported),
        &Field::Present(interfaces),
        limit,
        Liveness::Alive,
    ))
    .expect("the view reads");
    (view, calls)
}

#[test]
fn a_container_over_the_limit_is_never_listed() {
    let (view, calls) = read(5000, 4, Vec::new(), 10, 6);
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 0);
    assert_eq!(calls.child_at_index.load(Ordering::SeqCst), 4);
    assert_eq!(view.children.present().map(Vec::len), Some(4));
    let slots = view.slots.expect("a cut list carries its slots");
    assert_eq!(
        (slots.positions, slots.reported, slots.grid_view),
        (vec![0, 1, 2, 3], 5000, false)
    );
}

#[test]
fn a_container_at_the_limit_is_still_listed() {
    let (view, calls) = read(4, 4, Vec::new(), 10, 6);
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 1);
    assert_eq!(calls.child_at_index.load(Ordering::SeqCst), 0);
    assert!(view.slots.is_none());
    assert_eq!(view.children.present().map(Vec::len), Some(4));
}

#[test]
fn a_table_over_the_limit_presents_the_cells_in_view_with_their_own_indexes() {
    let interfaces = ["org.a11y.atspi.Component", "org.a11y.atspi.Table"]
        .map(str::to_owned)
        .to_vec();
    let (view, calls) = read(1_000_000, 4096, interfaces, 10, 6);
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 0);
    assert_eq!(calls.child_at_index.load(Ordering::SeqCst), 0);
    let slots = view.slots.expect("the view carries its slots");
    assert!(slots.grid_view);
    assert_eq!(slots.reported, 1_000_000);
    assert_eq!(slots.positions, (0..60).collect::<Vec<usize>>());
    let paths: Vec<String> = view
        .children
        .present()
        .expect("cells are listed")
        .iter()
        .map(|(_, path)| path.clone())
        .collect();
    assert_eq!(paths.first().map(String::as_str), Some("/cell/0"));
    assert_eq!(paths.last().map(String::as_str), Some("/cell/59"));
}

#[test]
fn the_view_stops_at_the_limit_row_by_row() {
    let interfaces = ["org.a11y.atspi.Component", "org.a11y.atspi.Table"]
        .map(str::to_owned)
        .to_vec();
    let (view, calls) = read(1_000_000, 25, interfaces, 10, 6);
    let slots = view.slots.expect("the view carries its slots");
    assert_eq!(slots.positions, (0..25).collect::<Vec<usize>>());
    assert!(calls.accessible_at.load(Ordering::SeqCst) >= 25);
}

#[test]
fn a_path_through_a_large_container_reads_one_child_only() {
    let (client, element, calls, _server) = grid(5000, 10, 6);
    let view = zbus::block_on(read_level_children(
        &client,
        &element,
        &Field::Present(5000),
        Some(37),
        Liveness::Alive,
    ))
    .expect("the level reads");
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 0);
    assert_eq!(calls.child_at_index.load(Ordering::SeqCst), 1);
    assert_eq!(view.slots.expect("slots").positions, vec![37]);
}

#[test]
fn the_shared_list_refuses_a_container_over_the_limit_without_asking_for_it() {
    let (client, element, calls, _server) = grid(5000, 10, 6);
    let error = zbus::block_on(list_children(&client, &element)).expect_err("too many children");
    assert_eq!(error.code, agent_desktop_core::ErrorCode::Internal);
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 0);
    let (client, element, calls, _server) = grid(8, 10, 6);
    let listed = zbus::block_on(list_children(&client, &element)).expect("a small list reads");
    assert_eq!(listed.len(), 8);
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 1);
    assert_eq!(element.path(), TABLE_PATH);
}

#[test]
fn a_table_without_component_falls_back_to_the_leading_children_and_is_cut() {
    let interfaces = vec!["org.a11y.atspi.Table".to_owned()];
    let (view, calls) = read(1_000_000, 6, interfaces, 10, 6);
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 0);
    assert_eq!(calls.accessible_at.load(Ordering::SeqCst), 0);
    assert_eq!(calls.child_at_index.load(Ordering::SeqCst), 6);
    let slots = view.slots.expect("a cut list carries its slots");
    assert!(!slots.grid_view);
    assert_eq!(slots.positions, (0..6).collect::<Vec<usize>>());
}

#[test]
fn the_hit_test_sees_the_cells_in_view_of_a_large_table_and_never_lists_it() {
    let (client, element, calls, _server) = grid(1_000_000, 10, 6);
    let (children, whole) =
        zbus::block_on(presented_children(&client, &element)).expect("children read");
    assert_eq!(children.len(), 60);
    assert!(!whole);
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 0);
}

#[test]
fn a_child_is_proven_by_its_own_index_in_a_container_over_the_limit() {
    let (client, parent, calls, _server) = grid(1_000_000, 10, 6);
    let cell = AtspiElement::new(BUS, "/cell/59", 1);
    let unserved = AtspiElement::new(BUS, "/cell/5", 1);
    assert!(
        zbus::block_on(is_child_at_own_index(&client, &parent, &cell)).expect("the index reads")
    );
    assert!(
        !zbus::block_on(is_child_at_own_index(&client, &parent, &unserved))
            .expect("a failed read is not an error")
    );
    assert_eq!(calls.get_children.load(Ordering::SeqCst), 0);
}
