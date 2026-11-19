//! Which display path draws a text node's caret. A window the GNOME Shell
//! extension places is drawn: a native Wayland window from the window
//! coordinates its toolkit reports, an Xwayland window from its X11 screen
//! coordinates. In an X11 session the screen coordinates are the screen's,
//! and the renderer draws the caret where they say.
use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

use crate::system::display::display_server::DisplayServer;
use crate::tree::placement::shell_place::ShellPlace;

/// The display path a caret read takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaretRoute {
    /// A window the extension places on the stage.
    ShellPlaced(ShellPlace),
    /// An X11 session, whose screen coordinates need no mapping.
    X11Screen,
    /// A Wayland window the extension did not place: the extension is
    /// missing, or the frame matched no single window.
    UnplacedWayland,
}

impl CaretRoute {
    /// The route for a node on `server` whose frame the extension placed by
    /// `shell`.
    pub(crate) fn of(server: &DisplayServer, shell: Option<ShellPlace>) -> Self {
        match (server, shell) {
            (DisplayServer::X11(_), _) => Self::X11Screen,
            (DisplayServer::Wayland, Some(shell)) => Self::ShellPlaced(shell),
            (DisplayServer::Wayland | DisplayServer::Unavailable(_), None)
            | (DisplayServer::Unavailable(_), Some(_)) => Self::UnplacedWayland,
        }
    }

    /// How the frame lands on the stage, `None` in an X11 session where it
    /// lands as it is, or the refusal for a Wayland window the extension did
    /// not place.
    pub(crate) fn place(self) -> Result<Option<ShellPlace>, AdapterError> {
        match self {
            Self::ShellPlaced(shell) => Ok(Some(shell)),
            Self::X11Screen => Ok(None),
            Self::UnplacedWayland => Err(AdapterError::new(
                ErrorCode::PlatformNotSupported,
                "The typing caret needs the window placed by the agent-desktop GNOME Shell extension",
            )
            .with_suggestion(
                "Install and enable the agent-desktop@agent-browser.dev extension from the matching release, then log in again",
            )
            .with_disposition(DeliverySemantics::not_delivered())),
        }
    }
}

#[cfg(test)]
#[path = "caret_route_tests.rs"]
mod tests;
