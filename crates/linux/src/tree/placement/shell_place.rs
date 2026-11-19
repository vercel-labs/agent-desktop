//! Where GNOME Shell puts a frame's node rectangles on the stage. A native
//! Wayland toolkit reports window coordinates, which start at the window's
//! place; an Xwayland toolkit reports X11 screen coordinates, which map onto
//! the stage by the Xwayland scale.
use super::shell_space::on_screen;
use super::visibility::is_sentinel;
use crate::system::shell::extension::protocol_map::ProtocolMap;
use crate::tree::Extents;

/// How the nodes of one frame land on the stage, as the GNOME Shell extension
/// reports the frame's window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellPlace {
    /// A native Wayland window whose window coordinates start at this stage
    /// point.
    Native((i32, i32)),
    /// An Xwayland window whose X11 coordinates land on the stage by this map.
    Xwayland(ProtocolMap),
}

impl ShellPlace {
    /// The rectangle a node is placed from: its window rectangle on a native
    /// window, its screen rectangle on an Xwayland one.
    pub(crate) fn source(
        self,
        screen: Option<Extents>,
        window: Option<Extents>,
    ) -> Option<Extents> {
        match self {
            Self::Native(_) => window,
            Self::Xwayland(_) => screen,
        }
        .filter(|rect| !is_sentinel(*rect))
    }

    /// A rectangle in the coordinates [`ShellPlace::source`] chose, on the
    /// stage.
    pub(crate) fn to_stage(self, rect: Extents) -> Option<Extents> {
        match self {
            Self::Native(origin) => on_screen(rect, origin),
            Self::Xwayland(map) => map.to_stage(rect),
        }
    }

    /// A node's stage rectangle from the screen and window rectangles its
    /// toolkit reports.
    pub(crate) fn place(self, screen: Option<Extents>, window: Option<Extents>) -> Option<Extents> {
        self.to_stage(self.source(screen, window)?)
    }
}

#[cfg(test)]
#[path = "shell_place_tests.rs"]
mod tests;
