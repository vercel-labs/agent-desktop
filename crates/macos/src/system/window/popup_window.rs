//! Whether an application shows a window that its accessibility window list
//! leaves out. AppKit draws a popover, an open menu and a sharing picker each
//! in a window of its own that `AXWindows` does not name: a popover at the
//! normal level, a menu or a picker at the pop-up menu level.

use rustc_hash::FxHashSet;

use super::cg_window::{bool_field, int_field, int_or_float_field, window_dictionaries};

/// `kCGMainMenuWindowLevel` to `kCGStatusWindowLevel`: the menu bar and its
/// status items, which are not surfaces of a window.
const MENU_BAR_LEVELS: std::ops::RangeInclusive<i64> = 24..=25;
/// `kCGPopUpMenuWindowLevel`.
const POPUP_MENU_WINDOW_LEVEL: i64 = 101;

/// Whether an on-screen window can hold a surface the accessibility window
/// list does not name. A closed popover leaves its window on screen at alpha 0.
fn holds_unlisted_surface(layer: i64, alpha: f64, number: i64, listed: &FxHashSet<i64>) -> bool {
    (0..=POPUP_MENU_WINDOW_LEVEL).contains(&layer)
        && !MENU_BAR_LEVELS.contains(&layer)
        && alpha > 0.0
        && !listed.contains(&number)
}

/// Whether `pid` has an on-screen window that none of `listed`, the window ids
/// of its accessibility windows, names. A window list that cannot be read says
/// yes, so a caller searches instead of missing a surface.
pub(crate) fn shows_unlisted_window(pid: i32, listed: &FxHashSet<i64>) -> bool {
    let Ok(windows) = window_dictionaries() else {
        return true;
    };
    windows.iter().any(|window| {
        int_field(window, "kCGWindowOwnerPID") == Some(i64::from(pid))
            && bool_field(window, "kCGWindowIsOnscreen").unwrap_or(true)
            && match (
                int_field(window, "kCGWindowLayer"),
                int_field(window, "kCGWindowNumber"),
            ) {
                (Some(layer), Some(number)) => holds_unlisted_surface(
                    layer,
                    int_or_float_field(window, "kCGWindowAlpha").unwrap_or(1.0),
                    number,
                    listed,
                ),
                _ => true,
            }
    })
}

#[cfg(test)]
#[path = "popup_window_tests.rs"]
mod tests;
