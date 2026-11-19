//! Which display path draws a text node's caret. A window the GNOME Shell
//! extension places is drawn: a native Wayland window from the window
//! coordinates its toolkit reports, an Xwayland window from its X11 screen
//! coordinates. An X11 session has its own arm, which refuses until its
//! renderer draws a caret.
use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

use crate::system::display::display_server::DisplayServer;
use crate::tree::placement::shell_place::ShellPlace;

/// The display path a caret read takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaretRoute {
    /// A window the extension places on the stage.
    ShellPlaced(ShellPlace),
    X11Session,
    /// A Wayland window the extension did not place: the extension is
    /// missing, or the frame matched no single window.
    UnplacedWayland,
}

impl CaretRoute {
    /// The route for a node on `server` whose frame the extension placed by
    /// `shell`.
    pub(crate) fn of(server: &DisplayServer, shell: Option<ShellPlace>) -> Self {
        match (server, shell) {
            (DisplayServer::X11(_), _) => Self::X11Session,
            (DisplayServer::Wayland, Some(shell)) => Self::ShellPlaced(shell),
            (DisplayServer::Wayland | DisplayServer::Unavailable(_), None)
            | (DisplayServer::Unavailable(_), Some(_)) => Self::UnplacedWayland,
        }
    }

    /// How the frame lands on the stage, or the refusal for a path that draws
    /// no caret.
    pub(crate) fn place(self) -> Result<ShellPlace, AdapterError> {
        let (message, suggestion) = match self {
            Self::ShellPlaced(shell) => return Ok(shell),
            Self::X11Session => (
                "The typing caret is not drawn in an X11 session",
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
