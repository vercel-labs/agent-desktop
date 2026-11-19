//! Where a cursor stands and how it got there.
use agent_desktop_core::Point;

use super::travel::Travel;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Motion {
    /// The tip, once the cursor has been placed.
    pub(crate) tip: Option<Point>,
    /// Whether the tip is where the agent left it. A hide that cancels a drag
    /// drops the anchor, so the next travel sets off near its destination
    /// instead of from the stale grab point.
    pub(crate) anchored: bool,
    /// Whether the last travel or effect drawn was a travel to where a drag
    /// begins.
    pub(crate) dragging: bool,
    /// Travels since the cursor was enabled, which vary each path's shape.
    pub(crate) moves: u64,
    /// Whether the pointer image is shown instead of the arrow.
    pub(crate) pointing: bool,
    pub(crate) travel: Option<Travel>,
}
