use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

use super::*;

fn marked_window(connection: &RustConnection, index: usize, by_renderer: bool) -> Window {
    let screen = &connection.setup().roots[index];
    let intern = |name: &[u8]| {
        connection
            .intern_atom(false, name)
            .unwrap()
            .reply()
            .unwrap()
            .atom
    };
    let window = connection.generate_id().unwrap();
    connection
        .create_window(
            screen.root_depth,
            window,
            screen.root,
            0,
            0,
            4,
            4,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new().override_redirect(1),
        )
        .unwrap();
    connection
        .change_property8(
            PropMode::REPLACE,
            window,
            intern(OWNER),
            AtomEnum::STRING,
            br#"["sweep-proof",null]"#,
        )
        .unwrap();
    if by_renderer {
        connection
            .change_property32(
                PropMode::REPLACE,
                window,
                intern(RENDERER),
                AtomEnum::CARDINAL,
                &[1],
            )
            .unwrap();
    }
    connection.get_input_focus().unwrap().reply().unwrap();
    window
}

#[test]
#[ignore = "needs a private X11 desktop; run through scripts/private-session.sh"]
fn live_sweep_removes_windows_left_behind_and_keeps_renderer_windows() {
    std::env::var("AGENT_DESKTOP_PRIVATE_SESSION").expect("private desktop");
    let display = std::env::var("DISPLAY").expect("display");
    let (oracle, index) = x11rb::connect(None).expect("oracle connection");
    let (leaver, leaver_index) = x11rb::connect(None).expect("connection that leaves a window");
    let left_behind = marked_window(&leaver, leaver_index, false);
    let drawn = marked_window(&oracle, index, true);
    Stage::open(&display)
        .expect("stage")
        .sweep_left_behind()
        .expect("sweep");
    let children = oracle
        .query_tree(oracle.setup().roots[index].root)
        .unwrap()
        .reply()
        .unwrap()
        .children;
    assert!(
        !children.contains(&left_behind),
        "a left-behind window goes"
    );
    assert!(children.contains(&drawn), "a renderer's window stays");
    oracle.destroy_window(drawn).unwrap();
    oracle.flush().unwrap();
}
