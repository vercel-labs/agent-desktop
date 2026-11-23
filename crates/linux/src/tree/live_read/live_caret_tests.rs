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
        origin: (311, 187),
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
        origin: (311, 187),
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
        origin: (311, 187),
    };
    assert_eq!(
        caret_on_screen(rect(139, 63, 2, 17), CoordSpace::Screen, frame),
        None
    );
    let unplaced = CaretFrame {
        screen: Some(rect(47, 61, 300, 21)),
        window: None,
        origin: (311, 187),
    };
    assert_eq!(
        caret_on_screen(rect(139, 63, 2, 17), CoordSpace::Screen, unplaced),
        None
    );
}
