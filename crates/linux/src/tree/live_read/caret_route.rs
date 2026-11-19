//! Which display path draws a text node's caret. Only a native Wayland window
//! the GNOME Shell extension places is drawn: its toolkit reports character
//! rectangles in window coordinates and the extension says where the window
//! sits. An X11 session and an Xwayland window have their own arms, which
//! refuse until their coordinate path is measured.
use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

use crate::system::display::display_server::DisplayServer;

/// The display path a caret read takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaretRoute {
    /// A native Wayland window whose window coordinates start at this screen
    /// point.
    ShellPlaced((i32, i32)),
    X11Session,
    XwaylandWindow,
    /// A Wayland window the extension did not place: the extension is
    /// missing, or the frame matched no single window.
    UnplacedWayland,
}

impl CaretRoute {
    /// The route for a node on `server` whose frame the extension placed at
    /// `shell_origin`. `on_xwayland` is asked only for a Wayland frame the
    /// extension did not place.
    pub(crate) fn of(
        server: &DisplayServer,
        shell_origin: Option<(i32, i32)>,
        on_xwayland: impl FnOnce() -> bool,
    ) -> Self {
        match (server, shell_origin) {
            (DisplayServer::X11(_), _) => Self::X11Session,
            (DisplayServer::Wayland, Some(origin)) => Self::ShellPlaced(origin),
            (DisplayServer::Wayland, None) => {
                if on_xwayland() {
                    Self::XwaylandWindow
                } else {
                    Self::UnplacedWayland
                }
            }
            (DisplayServer::Unavailable(_), _) => Self::UnplacedWayland,
        }
    }

    /// The screen point the window coordinates start at, or the refusal for a
    /// path that draws no caret.
    pub(crate) fn origin(self) -> Result<(i32, i32), AdapterError> {
        let (message, suggestion) = match self {
            Self::ShellPlaced(origin) => return Ok(origin),
            Self::X11Session => (
                "The typing caret is not drawn in an X11 session",
                "The cursor overlay still travels and clicks; only the caret is missing",
            ),
            Self::XwaylandWindow => (
                "The typing caret is not drawn over an Xwayland window",
                "The cursor overlay still travels and clicks; only the caret is missing",
            ),
            Self::UnplacedWayland => (
                "The typing caret needs the window placed by the agent-desktop GNOME Shell extension",
                "Install and enable the agent-desktop@agent-browser.dev extension from the matching release, then log in again",
            ),
        };
        Err(AdapterError::new(ErrorCode::PlatformNotSupported, message)
            .with_suggestion(suggestion)
            .with_disposition(DeliverySemantics::not_delivered()))
    }
}

#[cfg(test)]
#[path = "caret_route_tests.rs"]
mod tests;
