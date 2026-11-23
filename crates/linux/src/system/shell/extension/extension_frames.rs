//! The frames the extension plays, sampled from core's cursor motion. Core
//! owns the trajectory and the ripple timing, so the macOS, Windows and
//! Wayland overlays move the same way; the extension only picks the frame
//! for the time that has passed.
use agent_desktop_core::{CURSOR_HIGHLIGHT_HOLD_MS, CursorMotion};

/// The interval between frames, one display refresh at 60 Hz.
pub(crate) const FRAME_MS: f64 = 1_000.0 / 60.0;

const HIGHLIGHT_RISE: f64 = 0.08;
const HIGHLIGHT_FALL: f64 = 0.35;

fn sample_times(total_ms: u64) -> impl Iterator<Item = u64> {
    let count = (total_ms as f64 / FRAME_MS).ceil() as u64;
    (0..=count).map(move |frame| ((frame as f64 * FRAME_MS).round() as u64).min(total_ms))
}

/// The cursor's tip at every frame of a travel, ending on its destination
/// once the dwell before the action has passed.
pub(crate) fn travel_frames(motion: &CursorMotion) -> Vec<[f64; 2]> {
    sample_times(motion.total_ms())
        .map(|elapsed| {
            let point = motion.pose(elapsed).point;
            [point.x, point.y]
        })
        .collect()
}

/// The ripple phase and the highlight opacity at every frame of the click
/// flourish. Empty when there is neither a ripple nor a highlight to play.
pub(crate) fn effect_frames(motion: &CursorMotion, ripple: bool, highlight: bool) -> Vec<[f64; 2]> {
    if !ripple && !highlight {
        return Vec::new();
    }
    let ripple_ms = if ripple { motion.total_ms() } else { 0 };
    let highlight_ms = if highlight {
        CURSOR_HIGHLIGHT_HOLD_MS
    } else {
        0
    };
    sample_times(ripple_ms.max(highlight_ms))
        .map(|elapsed| {
            let phase = if ripple {
                motion.pose(elapsed).ripple
            } else {
                0.0
            };
            let opacity = if highlight {
                highlight_progress(elapsed, CURSOR_HIGHLIGHT_HOLD_MS)
            } else {
                0.0
            };
            [phase, opacity]
        })
        .collect()
}

/// The highlight's opacity across its hold: it rises, holds and falls, the
/// same envelope the Windows overlay plays, and is zero once the hold ends.
pub(crate) fn highlight_progress(elapsed_ms: u64, hold_ms: u64) -> f64 {
    if hold_ms == 0 || elapsed_ms >= hold_ms {
        return 0.0;
    }
    let fraction = elapsed_ms as f64 / hold_ms as f64;
    if fraction < HIGHLIGHT_RISE {
        return fraction / HIGHLIGHT_RISE;
    }
    if fraction > 1.0 - HIGHLIGHT_FALL {
        return (1.0 - fraction) / HIGHLIGHT_FALL;
    }
    1.0
}

#[cfg(test)]
#[path = "extension_frames_tests.rs"]
mod tests;
