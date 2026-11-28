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
fn a_window_that_could_be_the_front_is_shown_whole_and_not_a_tool() {
    assert!(can_be_front(&window(true, false, false, false)));
    assert!(!can_be_front(&window(true, false, true, false)));
    assert!(!can_be_front(&window(true, true, false, false)));
    assert!(!can_be_front(&window(false, false, false, false)));
    assert!(!can_be_front(&window(true, false, false, true)));
}
