use super::*;

fn client(id: u32, pid: u32, hidden: bool) -> X11Client {
    X11Client {
        shadow: (0, 0, 0, 0),
        id,
        hidden,
        pid: Some(pid),
        title: String::new(),
        extents: None,
        bounds: None,
    }
}

#[test]
fn the_top_window_is_the_last_visible_client_of_the_stacking_order() {
    let clients = [
        client(1, 10, false),
        client(2, 20, false),
        client(3, 30, true),
    ];
    let view = view_of(Some(1), &clients, &[1, 2, 3]);
    assert_eq!(view.focused.unwrap().id, "1");
    assert_eq!(view.top.unwrap().id, "2");
    assert_eq!(view.windows, ["1", "2", "3"]);
}

#[test]
fn a_new_window_of_an_unknown_process_is_listed_and_can_be_the_active_one() {
    let clients = [client(1, 10, false), client(9, 99, false)];
    let view = view_of(Some(9), &clients, &[1, 9]);
    assert_eq!(view.focused.unwrap().pid, 99);
    assert!(view.windows.contains(&"9".to_owned()));
}

#[test]
fn an_unreadable_stacking_order_leaves_the_top_window_unknown() {
    let view = view_of(Some(1), &[client(1, 10, false)], &[]);
    assert!(view.top.is_none());
    assert!(view.focused.is_some());
}

#[test]
fn a_window_id_that_is_not_an_x11_window_is_refused() {
    let window = FrontWindow {
        id: "w-1".into(),
        pid: 1,
    };
    assert!(client_of(&window).is_err());
}
