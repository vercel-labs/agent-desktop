use super::*;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn xwayland() -> ShellPlace {
    ShellPlace::Xwayland(
        ProtocolMap::new(rect(774, 504, 500, 338), rect(387, 252, 250, 169)).expect("map"),
    )
}

#[test]
fn a_native_node_is_placed_from_its_window_rectangle() {
    let place = ShellPlace::Native((311, 187));
    assert_eq!(
        place.place(Some(rect(0, 0, 120, 50)), Some(rect(47, 86, 120, 50))),
        Some(rect(358, 273, 120, 50))
    );
    assert_eq!(place.place(Some(rect(0, 0, 120, 50)), None), None);
}

#[test]
fn an_xwayland_node_is_placed_from_its_screen_rectangle() {
    assert_eq!(
        xwayland().place(Some(rect(854, 582, 420, 34)), Some(rect(40, 40, 420, 34))),
        Some(rect(427, 291, 210, 17))
    );
    assert_eq!(xwayland().place(None, Some(rect(40, 40, 420, 34))), None);
}

#[test]
fn a_sentinel_source_is_never_placed() {
    let sentinel = rect(i32::MIN, i32::MIN, -1, -1);
    assert_eq!(xwayland().place(Some(sentinel), None), None);
    assert_eq!(ShellPlace::Native((1, 1)).place(None, Some(sentinel)), None);
}
