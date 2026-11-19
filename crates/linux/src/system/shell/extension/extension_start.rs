use agent_desktop_core::Point;
use serde::Deserialize;

/// Where a travel sets off, as the extension reports it: the cursor's tip,
/// or a point near the destination for a cursor with no known position, and
/// how many travels the cursor has made since it was enabled, which varies
/// the shape of each path.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CursorStart {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) moves: u64,
}

impl CursorStart {
    pub(crate) const fn point(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
}
