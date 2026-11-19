//! What the cursors need to know about the screen they are drawn on.
use agent_desktop_core::{Point, Rect};

/// The display the renderer draws on: where the real pointer is and which
/// monitor holds a point. Tests stand in their own.
pub(crate) trait Desk {
    /// Where the real pointer is.
    fn pointer(&self) -> Point;
    /// The monitor that holds `at`, or the primary monitor when none does.
    fn monitor_at(&self, at: &Point) -> Rect;
    /// The part of that monitor windows may use, which keeps the label off
    /// panels and docks.
    fn work_area_at(&self, at: &Point) -> Rect;
}
