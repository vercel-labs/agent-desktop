//! The click ripple and the outline around the acted-on element.
use std::time::{Duration, Instant};

use agent_desktop_core::Rect;

use super::request::Frames;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Effect {
    /// Pairs of ripple phase and outline opacity, one per frame.
    pub(crate) path: Frames,
    pub(crate) started: Instant,
    pub(crate) target: Option<Rect>,
}

impl Effect {
    /// The ripple phase and outline opacity at `now`, or `None` once the
    /// effect has played, as the extension stops it on completion.
    pub(crate) fn at(&self, now: Instant) -> Option<[f64; 2]> {
        let elapsed = now.saturating_duration_since(self.started);
        let length = self.path.length().max(Duration::from_millis(1));
        if elapsed >= length {
            return None;
        }
        self.path.at(elapsed)
    }
}
