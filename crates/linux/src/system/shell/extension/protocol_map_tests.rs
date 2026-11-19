use super::*;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn scaled_twice() -> ProtocolMap {
    ProtocolMap::new(rect(774, 504, 500, 338), rect(387, 252, 250, 169)).expect("map")
}

#[test]
fn an_xwayland_rectangle_at_twice_the_stage_scale_lands_at_half_size() {
    assert_eq!(
        scaled_twice().to_stage(rect(854, 582, 420, 34)),
        Some(rect(427, 291, 210, 17))
    );
}

#[test]
fn an_unscaled_xwayland_window_keeps_its_rectangles() {
    let map = ProtocolMap::new(rect(390, 248, 500, 337), rect(390, 248, 500, 337)).expect("map");
    assert_eq!(
        map.to_stage(rect(437, 309, 300, 21)),
        Some(rect(437, 309, 300, 21))
    );
}

#[test]
fn odd_edges_round_to_the_nearest_stage_unit_and_neighbours_still_touch() {
    let map = scaled_twice();
    let left = map.to_stage(rect(775, 505, 3, 19)).expect("left");
    let right = map.to_stage(rect(778, 505, 3, 19)).expect("right");
    assert_eq!(left, rect(388, 253, 1, 9));
    assert_eq!(left.x + left.width, right.x);
}

#[test]
fn a_rectangle_left_of_the_frame_maps_by_the_same_scale() {
    assert_eq!(
        scaled_twice().to_stage(rect(0, 0, 40, 40)),
        Some(rect(0, 0, 20, 20))
    );
}

#[test]
fn an_empty_frame_gives_no_map() {
    assert_eq!(ProtocolMap::new(rect(0, 0, 0, 0), rect(10, 10, 5, 5)), None);
    assert_eq!(ProtocolMap::new(rect(10, 10, 5, 5), rect(0, 0, 5, 0)), None);
}

#[test]
fn maps_of_two_windows_of_one_xwayland_agree() {
    let other = ProtocolMap::new(rect(200, 100, 640, 480), rect(100, 50, 320, 240)).expect("map");
    assert!(scaled_twice().agrees(other));
    let unscaled =
        ProtocolMap::new(rect(200, 100, 640, 480), rect(200, 100, 640, 480)).expect("map");
    assert!(!scaled_twice().agrees(unscaled));
}

#[test]
fn an_overflowing_rectangle_has_no_stage_position() {
    assert_eq!(scaled_twice().to_stage(rect(i32::MAX, 0, 10, 10)), None);
}
