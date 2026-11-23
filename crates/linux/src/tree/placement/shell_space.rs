//! Placement of windows on a GNOME Wayland session through the GNOME Shell
//! extension. A native Wayland client cannot learn where its window sits, so
//! its toolkit reports window coordinates even where it claims screen ones;
//! the extension supplies the window's place, and every node rectangle moves
//! by it. An Xwayland client reports X11 screen coordinates, which the
//! extension's frames map onto the stage.
use agent_desktop_core::{BoundsSpace, Deadline};

use super::shell_place::ShellPlace;
use super::visibility::is_sentinel;
use crate::system::display::display_server::{DisplayServer, session_server};
use crate::system::shell::extension::extension_place::{
    live_windows, matching_window, xwayland_map,
};
use crate::system::shell::extension::protocol_map::ProtocolMap;
use crate::system::within;
use crate::tree::properties::node_read::read_node_for;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::surfaces::window_extents::read_window_extents_async;
use crate::tree::{AtspiElement, Extents, Field, ObservedRead};

/// The title and window-coordinate rectangle of `element`'s frame.
fn frame_facts(
    connection: &zbus::Connection,
    element: &AtspiElement,
    deadline: Deadline,
) -> Option<(String, Option<Extents>)> {
    let frame = element.frame()?;
    let (title, window) = within(deadline, async {
        let node = read_node_for(connection, frame, ReadPurpose::Frame)
            .await
            .ok()
            .map(|(node, _)| node);
        let window = read_window_extents_async(connection, frame)
            .await
            .ok()
            .flatten();
        Ok((
            node.and_then(|node| node.properties.name.present().cloned()),
            window,
        ))
    })
    .ok()?;
    Some((title.unwrap_or_default(), window))
}

/// How the GNOME Shell extension places `element`'s frame on the stage: by
/// the origin of the one native Wayland window it matches, or by the X11 map
/// of its process's Xwayland windows. `None` on X11, for an element without a
/// frame, without the extension, and for a frame neither places exactly.
pub(crate) fn frame_place(
    connection: &zbus::Connection,
    element: &AtspiElement,
    deadline: Deadline,
) -> Option<ShellPlace> {
    if session_server() != DisplayServer::Wayland {
        return None;
    }
    let pid = element.frame()?.pid();
    let (title, window) = frame_facts(connection, element, deadline)?;
    let windows = live_windows(deadline)?;
    let native = window
        .filter(|window| !is_sentinel(*window))
        .and_then(|window| matching_window(&windows, pid, &title, (window.width, window.height)));
    if let Some((_, space)) = native {
        return Some(ShellPlace::Native((space.x, space.y)));
    }
    xwayland_map(&windows, pid).map(ShellPlace::Xwayland)
}

/// A window-coordinate rectangle moved to the frame's place on screen.
pub(crate) fn on_screen(window: Extents, origin: (i32, i32)) -> Option<Extents> {
    if is_sentinel(window) {
        return None;
    }
    Some(Extents {
        x: window.x.checked_add(origin.0)?,
        y: window.y.checked_add(origin.1)?,
        width: window.width,
        height: window.height,
    })
}

/// Places one read on a native Wayland window from its window-coordinate
/// rectangle: the screen rectangle is that rectangle moved to `origin`, and
/// the window rectangle stays the evidence, tagged with its frame.
pub(super) fn place_read(read: &mut ObservedRead, origin: (i32, i32), window: Option<Extents>) {
    let window = window.filter(|window| !is_sentinel(*window));
    read.evidence_bounds = window.map(|window| (window, BoundsSpace::Window));
    read.node.extents = window
        .and_then(|window| on_screen(window, origin))
        .map_or_else(
            || Field::Failed("Screen position could not be established".to_owned()),
            Field::Present,
        );
}

/// Places one read on an Xwayland window: its screen rectangle, in X11
/// coordinates, moves onto the stage.
pub(super) fn map_read(read: &mut ObservedRead, map: ProtocolMap) {
    let Field::Present(screen) = read.node.extents else {
        return;
    };
    if is_sentinel(screen) {
        return;
    }
    read.node.extents = map.to_stage(screen).map_or_else(
        || Field::Failed("Screen position could not be established".to_owned()),
        Field::Present,
    );
}

#[cfg(test)]
#[path = "shell_space_tests.rs"]
mod tests;
