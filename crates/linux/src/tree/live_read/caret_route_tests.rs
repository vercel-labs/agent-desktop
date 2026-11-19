use super::*;
use crate::system::display::display_cause::Cause;
use crate::system::display::display_unavailable::Unavailable;
use crate::system::shell::extension::protocol_map::ProtocolMap;
use crate::tree::Extents;

fn unavailable() -> DisplayServer {
    DisplayServer::Unavailable(Unavailable {
        cause: Cause::NotConfigured,
        detail: String::new(),
    })
}

fn xwayland() -> ShellPlace {
    let frame = Extents {
        x: 387,
        y: 252,
        width: 250,
        height: 169,
    };
    let protocol = Extents {
        x: 774,
        y: 504,
        width: 500,
        height: 338,
    };
    ShellPlace::Xwayland(ProtocolMap::new(protocol, frame).expect("map"))
}

#[test]
fn a_shell_placed_window_draws_by_its_place() {
    let native = CaretRoute::of(
        &DisplayServer::Wayland,
        Some(ShellPlace::Native((311, 187))),
    );
    assert_eq!(
        native.place().expect("place"),
        Some(ShellPlace::Native((311, 187)))
    );
    let xwayland_route = CaretRoute::of(&DisplayServer::Wayland, Some(xwayland()));
    assert_eq!(xwayland_route.place().expect("place"), Some(xwayland()));
}

#[test]
fn an_x11_session_draws_in_screen_coordinates_whatever_the_shell_says() {
    let route = CaretRoute::of(&DisplayServer::X11(":0".into()), Some(xwayland()));
    assert_eq!(route, CaretRoute::X11Screen);
    assert_eq!(route.place().expect("drawn"), None);
}

#[test]
fn a_frame_the_extension_did_not_place_names_the_extension() {
    let route = CaretRoute::of(&DisplayServer::Wayland, None);
    assert_eq!(route, CaretRoute::UnplacedWayland);
    let error = route.place().expect_err("refused");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert!(error.message.contains("extension"), "{}", error.message);
    assert_eq!(
        CaretRoute::of(&unavailable(), Some(xwayland())),
        CaretRoute::UnplacedWayland
    );
}
