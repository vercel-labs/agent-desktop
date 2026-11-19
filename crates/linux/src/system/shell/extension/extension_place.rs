use std::time::Duration;

use agent_desktop_core::Deadline;

use super::extension_window::{ShellWindow, list_windows};
use super::protocol_map::ProtocolMap;
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
/// then refused. Xwayland windows are skipped: [`xwayland_map`] places them
/// from the screen positions their toolkits report.
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

/// The one Xwayland window of process `pid` titled `title`, when it is not
/// minimized. A minimized window of the same title still counts, so its
/// visible twin's frame is never lent to it.
pub(crate) fn titled_xwayland<'a>(
    windows: &'a [ShellWindow],
    pid: u32,
    title: &str,
) -> Option<&'a ShellWindow> {
    let mut found = windows
        .iter()
        .filter(|window| !window.wayland && window.pid == pid && window.title == title);
    let first = found.next()?;
    (found.next().is_none() && !first.minimized).then_some(first)
}

/// How the X11 coordinates of process `pid` land on the stage, when its
/// windows are Xwayland windows. `None` when the process has a native Wayland
/// window, has no Xwayland window with both frames, or its windows disagree.
///
/// A process is either an X11 or a Wayland client, so the map is found by
/// process alone, and an untitled popup or dialog of the process maps the
/// same way as its main window.
pub(crate) fn xwayland_map(windows: &[ShellWindow], pid: u32) -> Option<ProtocolMap> {
    let mut owned = windows.iter().filter(|window| window.pid == pid).peekable();
    owned.peek()?;
    let mut maps = Vec::new();
    for window in owned {
        if window.wayland {
            return None;
        }
        if let Some(map) = window
            .protocol
            .and_then(|protocol| ProtocolMap::new(protocol, window.frame))
        {
            maps.push(map);
        }
    }
    let (first, rest) = maps.split_first()?;
    rest.iter().all(|map| first.agrees(*map)).then_some(*first)
}

#[cfg(test)]
#[path = "extension_place_tests.rs"]
mod tests;
