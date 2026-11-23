//! The cursor's label and when it was revealed. A new label waits, unseen,
//! until the cursor arrives, then eases in.
use std::time::{Duration, Instant};

/// How long a revealed label takes to ease in.
pub(crate) const LABEL_REVEAL: Duration = Duration::from_millis(180);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CursorLabel {
    pub(crate) text: Option<String>,
    /// When the label started to ease in; `None` while it waits.
    pub(crate) revealed: Option<Instant>,
}

impl CursorLabel {
    /// Sets the text. The same text changes nothing; a new one waits for the
    /// next reveal.
    pub(crate) fn set(&mut self, text: Option<String>) {
        if text != self.text {
            self.text = text;
            self.revealed = None;
        }
    }

    /// The label's opacity at `now`, eased out with a cubic as St eases it.
    pub(crate) fn opacity(&self, now: Instant) -> f64 {
        let Some(revealed) = self.revealed else {
            return 0.0;
        };
        let progress =
            now.saturating_duration_since(revealed).as_secs_f64() / LABEL_REVEAL.as_secs_f64();
        1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3)
    }

    /// Whether the label is still easing in at `now`.
    pub(crate) fn is_easing(&self, now: Instant) -> bool {
        self.revealed
            .is_some_and(|revealed| now.saturating_duration_since(revealed) < LABEL_REVEAL)
    }

    /// Starts easing in a label that waits.
    pub(crate) fn reveal(&mut self, now: Instant) {
        if self.text.is_some() && self.revealed.is_none() {
            self.revealed = Some(now);
        }
    }
}
