//! Whether a cursor is shown, and how long since the agent last used it.
use std::time::{Duration, Instant};

/// How long a cursor stays after the agent's last request.
pub(crate) const IDLE_REST: Duration = Duration::from_millis(6000);
/// How long it then takes to fade out.
pub(crate) const REST_FADE: Duration = Duration::from_millis(156);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Presence {
    /// Hidden by `hide` until `show`.
    pub(crate) hidden: bool,
    /// Hidden for a screenshot.
    pub(crate) concealed: bool,
    /// When the agent last used the cursor.
    pub(crate) woke: Instant,
}

impl Presence {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            hidden: false,
            concealed: false,
            woke: now,
        }
    }

    pub(crate) fn is_shown(&self) -> bool {
        !self.hidden && !self.concealed
    }

    /// Whether the cursor has started to rest at `now`.
    pub(crate) fn is_resting(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.woke) >= IDLE_REST
    }

    /// Whether the rest fade has finished at `now`.
    pub(crate) fn has_rested(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.woke) >= IDLE_REST + REST_FADE
    }

    /// The cursor's opacity at `now`: whole until it rests, then fading.
    pub(crate) fn opacity(&self, now: Instant) -> f64 {
        let idle = now.saturating_duration_since(self.woke);
        let fading = idle.saturating_sub(IDLE_REST).as_secs_f64();
        (1.0 - fading / REST_FADE.as_secs_f64()).clamp(0.0, 1.0)
    }
}
