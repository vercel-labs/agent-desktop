use super::*;

fn window(number: i64, pid: i32, on_screen: bool) -> OrderedWindow {
    OrderedWindow {
        number,
        pid,
        on_screen,
    }
}

#[test]
fn the_focused_window_is_the_frontmost_applications_topmost_one_on_screen() {
    let order = [
        window(9, 300, true),
        window(7, 200, false),
        window(5, 200, true),
        window(3, 200, true),
    ];
    let view = view_of(&order, Some(200));
    assert_eq!(view.focused.unwrap().id, "w-5");
    assert_eq!(view.top.unwrap().id, "w-9");
    assert_eq!(view.windows, ["w-9", "w-7", "w-5", "w-3"]);
}

#[test]
fn an_intruder_above_the_frontmost_application_shows_as_the_top_window() {
    let order = [window(11, 900, true), window(5, 200, true)];
    let view = view_of(&order, Some(200));
    assert_eq!(view.focused.unwrap().pid, 200);
    assert_eq!(view.top.unwrap().pid, 900);
}

#[test]
fn no_frontmost_application_means_no_focused_window() {
    let view = view_of(&[window(5, 200, true)], None);
    assert!(view.focused.is_none());
}
