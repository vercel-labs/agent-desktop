use std::time::Duration;

use agent_desktop_core::Deadline;

use super::extension_window::{ShellWindow, list_windows};
use crate::system::display::display_server::{DisplayServer, session_server};
use crate::tree::Extents;

/// The share of a caller's budget the window list may take, so a shell that
/// stops answering leaves geometry unknown instead of failing the read.
const LIST_SLICE: Duration = Duration::from_millis(500);

fn space_of(window: &ShellWindow, size: (i32, i32)) -> Option<Extents> {
    [window.frame, window.buffer]
        .into_iter()
        .find(|rect| (rect.width, rect.height) == size)
}

/// The one visible native Wayland window of process `pid` titled `title`
/// whose frame or buffer has `size`, paired with that rectangle.
///
/// A toolkit measures window coordinates from either the visible frame
/// (GTK 4) or its whole surface including client-side shadows (GTK 3, Qt,
/// Chromium), and the AT-SPI frame's own size says which: the rectangle of the
/// same size is the one its coordinates start from (`probes/linux/FINDINGS.md`
/// L43-2). Several matching windows, or none, give `None`, so a frame is never
/// placed on a guess. A minimized window still counts as a match, so a visible
/// window is never lent to the frame of a minimized twin; the match itself is
/// then refused. Xwayland windows are skipped, because their toolkits already
/// report global positions.
pub(crate) fn matching_window<'a>(
    windows: &'a [ShellWindow],
    pid: u32,
    title: &str,
    size: (i32, i32),
) -> Option<(&'a ShellWindow, Extents)> {
    if size.0 <= 0 || size.1 <= 0 {
        return None;
    }
    let mut found = windows
        .iter()
        .filter(|window| window.wayland && window.pid == pid && window.title == title)
        .filter_map(|window| space_of(window, size).map(|space| (window, space)));
    let first = found.next()?;
    (found.next().is_none() && !first.0.minimized).then_some(first)
}

/// GNOME Shell's window list on a Wayland session, within a share of
/// `deadline`. `None` on X11, without the extension, or when it is late.
pub(crate) fn live_windows(deadline: Deadline) -> Option<Vec<ShellWindow>> {
    if session_server() != DisplayServer::Wayland {
        return None;
    }
    list_windows(deadline.capped(LIST_SLICE)).ok()?.ok()
}

/// Where the window coordinates of a native Wayland frame start on screen:
/// the rectangle [`matching_window`] chose from the live window list.
pub(crate) fn locate_frame(
    pid: u32,
    title: &str,
    size: (i32, i32),
    deadline: Deadline,
) -> Option<Extents> {
    let windows = live_windows(deadline)?;
    matching_window(&windows, pid, title, size).map(|(_, space)| space)
}

/// Whether a window of process `pid` titled `title` is an Xwayland window.
/// The extension places only native Wayland windows, so this tells a frame it
/// cannot place because it is X11 from one it found no window for.
pub(crate) fn xwayland_window(windows: &[ShellWindow], pid: u32, title: &str) -> bool {
    windows
        .iter()
        .any(|window| !window.wayland && window.pid == pid && window.title == title)
}

/// [`xwayland_window`] against the live window list. `false` when the list
/// cannot be read.
pub(crate) fn is_xwayland_frame(pid: u32, title: &str, deadline: Deadline) -> bool {
    live_windows(deadline).is_some_and(|windows| xwayland_window(&windows, pid, title))
}

#[cfg(test)]
#[path = "extension_place_tests.rs"]
mod tests;
