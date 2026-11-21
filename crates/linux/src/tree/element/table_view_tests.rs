use super::*;
use crate::tree::test_support::grid_fake::{Quirks, grid, grid_with};

#[test]
fn the_rectangle_between_two_corners_runs_row_by_row() {
    assert_eq!(
        rectangle((1, 2), (2, 3), 10),
        vec![(1, 2), (1, 3), (2, 2), (2, 3)]
    );
    assert_eq!(rectangle((2, 3), (1, 2), 3), vec![(1, 2), (1, 3), (2, 2)]);
}

#[test]
fn a_table_that_shows_no_cell_at_its_corners_presents_no_view() {
    let (client, element, _calls, _server) = grid_with(
        1_000_000,
        10,
        6,
        Quirks {
            blind: true,
            ..Quirks::default()
        },
    );
    let view = zbus::block_on(read_table_view(&client, &element, 4096, Liveness::Alive));
    assert!(matches!(view, Ok(None)), "{view:?}");
}

#[test]
fn a_hit_that_is_not_a_child_of_the_table_presents_no_view() {
    let (client, element, _calls, _server) = grid_with(
        1_000_000,
        10,
        6,
        Quirks {
            nested_hit: true,
            ..Quirks::default()
        },
    );
    let view = zbus::block_on(read_table_view(&client, &element, 4096, Liveness::Alive));
    assert!(matches!(view, Ok(None)), "{view:?}");
}

#[test]
fn a_table_whose_extents_cannot_be_read_presents_no_view() {
    let (client, element, _calls, _server) = grid(1_000_000, 10, 6);
    let missing = AtspiElement::new(element.bus_name(), "/missing", 1);
    let view = zbus::block_on(read_table_view(&client, &missing, 4096, Liveness::Alive));
    assert!(matches!(view, Ok(None)), "{view:?}");
}

#[test]
fn the_view_of_a_table_is_its_cells_with_their_indexes() {
    let (client, element, _calls, _server) = grid(1_000_000, 10, 6);
    let (cells, whole) = zbus::block_on(read_table_view(&client, &element, 4096, Liveness::Alive))
        .expect("the view reads")
        .expect("the table has a view");
    assert!(whole);
    assert_eq!(cells.len(), 60);
    assert_eq!(cells[11].1, 11);
    assert_eq!(cells[11].0.1, "/cell/11");
}

#[test]
fn a_cell_that_cannot_be_read_leaves_the_view_cut_short() {
    let (client, element, _calls, _server) = grid_with(
        1_000_000,
        10,
        6,
        Quirks {
            failing_cell: Some(11),
            ..Quirks::default()
        },
    );
    let (cells, whole) = zbus::block_on(read_table_view(&client, &element, 4096, Liveness::Alive))
        .expect("the view reads")
        .expect("the table has a view");
    assert!(!whole);
    assert_eq!(cells.len(), 59);
}

#[test]
fn a_view_cut_at_the_limit_is_not_the_whole_view() {
    let (client, element, _calls, _server) = grid(1_000_000, 10, 6);
    let (cells, whole) = zbus::block_on(read_table_view(&client, &element, 59, Liveness::Alive))
        .expect("the view reads")
        .expect("the table has a view");
    assert!(!whole);
    assert_eq!(cells.len(), 59);
}

#[test]
fn a_toolkit_that_answers_with_new_cell_objects_still_has_a_view() {
    let (client, element, _calls, _server) = grid_with(
        1_000_000,
        10,
        6,
        Quirks {
            fresh_objects: true,
            ..Quirks::default()
        },
    );
    let (cells, _) = zbus::block_on(read_table_view(&client, &element, 4096, Liveness::Alive))
        .expect("the view reads")
        .expect("the table has a view");
    assert_eq!(cells.len(), 60);
    assert_eq!(cells[0].0.1, "/fresh/0");
}
