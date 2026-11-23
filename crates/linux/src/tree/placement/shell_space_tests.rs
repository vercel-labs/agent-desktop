use super::*;
use crate::tree::live::test_support::read_of;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_window_rectangle_moves_by_the_frame_origin() {
    assert_eq!(
        on_screen(rect(47, 86, 120, 50), (311, 187)),
        Some(rect(358, 273, 120, 50))
    );
}

#[test]
fn a_sentinel_rectangle_has_no_screen_position() {
    assert_eq!(
        on_screen(rect(i32::MIN, i32::MIN, -1, -1), (311, 187)),
        None
    );
}

#[test]
fn an_overflowing_move_has_no_screen_position() {
    assert_eq!(on_screen(rect(i32::MAX, 0, 10, 10), (1, 0)), None);
}

#[test]
fn a_placed_read_takes_the_moved_rectangle_and_keeps_the_window_one_as_evidence() {
    let mut read = read_of(62, "Save", &[]);
    read.node.extents = Field::Present(rect(0, 0, 120, 50));
    place_read(&mut read, (311, 187), Some(rect(47, 86, 120, 50)));
    assert_eq!(read.node.extents, Field::Present(rect(358, 273, 120, 50)));
    assert_eq!(
        read.evidence_bounds,
        Some((rect(47, 86, 120, 50), BoundsSpace::Window))
    );
}

#[test]
fn a_read_whose_window_rectangle_is_unknown_loses_its_screen_position() {
    let mut read = read_of(62, "Save", &[]);
    read.node.extents = Field::Present(rect(0, 0, 120, 50));
    place_read(&mut read, (311, 187), None);
    assert!(matches!(read.node.extents, Field::Failed(_)));
    assert_eq!(read.evidence_bounds, None);
}
