use agent_desktop_core::Rect;

use super::*;

fn window(visible: bool, iconic: bool, tool: bool, cloaked: bool) -> EnumeratedWindow {
    EnumeratedWindow {
        handle: std::ptr::null_mut(),
        visible,
        iconic,
        cloaked,
        tool,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
    }
}

#[test]
fn the_top_window_is_the_first_one_a_user_could_see_in_front() {
    let order = [
        window(true, false, true, false),
        window(true, true, false, false),
        window(false, false, false, false),
        window(true, false, false, true),
        window(true, false, false, false),
    ];
    let (top, listed) = order_of(&order);
    assert_eq!(top, Some(4));
    assert_eq!(listed, [0, 1, 4]);
}

#[test]
fn a_desktop_with_nothing_to_see_has_no_top_window() {
    let (top, listed) = order_of(&[window(true, true, false, false)]);
    assert_eq!(top, None);
    assert_eq!(listed, [0]);
}
