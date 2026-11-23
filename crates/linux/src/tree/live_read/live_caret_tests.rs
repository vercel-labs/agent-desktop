use super::*;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn screen_rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_window_coordinate_caret_moves_by_the_window_origin() {
    let frame = CaretFrame {
        screen: None,
        window: Some(rect(40, 150, 420, 100)),
        place: ShellPlace::Native((311, 187)),
    };
    assert_eq!(
        caret_on_screen(rect(69, 175, 2, 20), CoordSpace::Window, frame),
        Some(screen_rect(380.0, 362.0, 2.0, 20.0))
    );
}

#[test]
fn a_screen_coordinate_caret_keeps_its_offset_from_the_node() {
    let frame = CaretFrame {
        screen: Some(rect(47, 61, 300, 21)),
        window: Some(rect(73, 140, 300, 21)),
        place: ShellPlace::Native((311, 187)),
    };
    assert_eq!(
        caret_on_screen(rect(139, 63, 2, 17), CoordSpace::Screen, frame),
        Some(screen_rect(476.0, 329.0, 2.0, 17.0))
    );
}

#[test]
fn a_node_without_a_rectangle_in_the_caret_space_places_nothing() {
    let frame = CaretFrame {
        screen: None,
        window: Some(rect(73, 140, 300, 21)),
        place: ShellPlace::Native((311, 187)),
    };
    assert_eq!(
        caret_on_screen(rect(139, 63, 2, 17), CoordSpace::Screen, frame),
        None
    );
    let unplaced = CaretFrame {
        screen: Some(rect(47, 61, 300, 21)),
        window: None,
        place: ShellPlace::Native((311, 187)),
    };
    assert_eq!(
        caret_on_screen(rect(139, 63, 2, 17), CoordSpace::Screen, unplaced),
        None
    );
}

#[test]
fn an_xwayland_caret_keeps_its_x11_offset_and_lands_scaled() {
    let map = crate::system::shell::extension::protocol_map::ProtocolMap::new(
        rect(774, 504, 500, 338),
        rect(387, 252, 250, 169),
    )
    .expect("map");
    let frame = CaretFrame {
        screen: Some(rect(814, 542, 420, 34)),
        window: Some(rect(40, 38, 420, 34)),
        place: ShellPlace::Xwayland(map),
    };
    assert_eq!(
        caret_on_screen(rect(893, 549, 2, 19), CoordSpace::Screen, frame),
        Some(screen_rect(447.0, 275.0, 2.0, 10.0))
    );
    assert_eq!(
        caret_on_screen(rect(119, 45, 2, 19), CoordSpace::Window, frame),
        Some(screen_rect(447.0, 275.0, 2.0, 10.0))
    );
}

#[test]
fn an_x11_caret_read_in_window_coordinates_lands_by_the_node_on_screen() {
    let caret = caret_on_x11_screen(
        rect(69, 175, 2, 20),
        CoordSpace::Window,
        Some(rect(0, 0, 420, 100)),
        Some(rect(40, 150, 420, 100)),
        rect(1040, 650, 420, 100),
    );
    assert_eq!(caret, Some(Rect::from(drawn_caret(rect(1069, 675, 2, 20)))));
}

#[test]
fn an_x11_caret_read_in_screen_coordinates_keeps_its_place_on_a_global_toolkit() {
    let node = rect(480, 290, 200, 30);
    let caret = caret_on_x11_screen(
        rect(500, 296, 1, 18),
        CoordSpace::Screen,
        Some(node),
        None,
        node,
    );
    assert_eq!(caret, Some(Rect::from(drawn_caret(rect(500, 296, 1, 18)))));
}

#[test]
fn an_x11_caret_without_the_node_rectangle_of_its_space_is_not_placed() {
    let node = rect(480, 290, 200, 30);
    assert_eq!(
        caret_on_x11_screen(
            rect(500, 296, 1, 18),
            CoordSpace::Window,
            Some(node),
            None,
            node
        ),
        None
    );
}
