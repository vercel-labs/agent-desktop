//! A cursor on its way along core's motion path, and the connection that
//! waits for it to arrive.
use std::time::Instant;

use agent_desktop_core::Point;

use super::request::Frames;

/// The connection a request waits on, answered when the request is done.
pub(crate) type Ticket = u64;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Travel {
    pub(crate) path: Frames,
    pub(crate) started: Instant,
    pub(crate) pointer: bool,
    pub(crate) ticket: Ticket,
}

fn point(frame: [f64; 2]) -> Point {
    Point {
        x: frame[0],
        y: frame[1],
    }
}

impl Travel {
    /// Whether the last frame is due at `now`.
    pub(crate) fn has_arrived(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) >= self.path.length()
    }

    /// Where the tip is at `now`.
    pub(crate) fn at(&self, now: Instant) -> Option<Point> {
        self.path
            .at(now.saturating_duration_since(self.started))
            .map(point)
    }

    /// Where the travel ends.
    pub(crate) fn destination(&self) -> Option<Point> {
        self.path.frames.last().copied().map(point)
    }
}
