use super::*;
use crate::system::display::display_cause::Cause;
use crate::system::display::display_unavailable::Unavailable;

fn unavailable() -> DisplayServer {
    DisplayServer::Unavailable(Unavailable {
        cause: Cause::NotConfigured,
        detail: String::new(),
    })
}

#[test]
fn a_shell_placed_native_window_draws_from_its_origin() {
    let route = CaretRoute::of(&DisplayServer::Wayland, Some((311, 187)), || {
        panic!("a placed window is not asked about Xwayland")
    });
    assert_eq!(route, CaretRoute::ShellPlaced((311, 187)));
    assert_eq!(route.origin().expect("origin"), (311, 187));
}

#[test]
fn an_x11_session_refuses_without_asking_the_shell() {
    let route = CaretRoute::of(&DisplayServer::X11(":0".into()), Some((5, 5)), || {
        panic!("an X11 session is not asked about Xwayland")
    });
    assert_eq!(route, CaretRoute::X11Session);
    let error = route.origin().expect_err("refused");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert!(error.message.contains("X11 session"), "{}", error.message);
}

#[test]
fn an_unplaced_xwayland_frame_takes_its_own_arm() {
    let route = CaretRoute::of(&DisplayServer::Wayland, None, || true);
    assert_eq!(route, CaretRoute::XwaylandWindow);
    let error = route.origin().expect_err("refused");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert!(error.message.contains("Xwayland"), "{}", error.message);
}

#[test]
fn a_native_frame_the_extension_did_not_place_names_the_extension() {
    let route = CaretRoute::of(&DisplayServer::Wayland, None, || false);
    assert_eq!(route, CaretRoute::UnplacedWayland);
    let error = route.origin().expect_err("refused");
    assert!(error.message.contains("extension"), "{}", error.message);
    assert_eq!(
        CaretRoute::of(&unavailable(), None, || panic!("no session")),
        CaretRoute::UnplacedWayland
    );
}
