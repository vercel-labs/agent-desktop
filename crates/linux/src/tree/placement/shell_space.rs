//! Placement of native Wayland windows through the GNOME Shell extension. A
//! Wayland client cannot learn where its window sits, so its toolkit reports
//! window coordinates even where it claims screen ones; the extension supplies
//! the window's place, and every node rectangle moves by it.
use agent_desktop_core::{BoundsSpace, Deadline};

use super::visibility::is_sentinel;
use crate::system::display::display_server::{DisplayServer, session_server};
use crate::system::shell::extension::extension_place::{is_xwayland_frame, locate_frame};
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

/// Where the window coordinates of `element`'s frame start on screen, as the
/// extension places the frame's native Wayland window. `None` on X11, for an
/// element without a frame, without the extension, and for a frame that does
/// not match exactly one window.
pub(crate) fn frame_origin(
    connection: &zbus::Connection,
    element: &AtspiElement,
    deadline: Deadline,
) -> Option<(i32, i32)> {
    if session_server() != DisplayServer::Wayland {
        return None;
    }
    let (title, window) = frame_facts(connection, element, deadline)?;
    let window = window.filter(|window| !is_sentinel(*window))?;
    let space = locate_frame(
        element.frame()?.pid(),
        &title,
        (window.width, window.height),
        deadline,
    )?;
    Some((space.x, space.y))
}

/// Whether `element`'s frame is an Xwayland window on a Wayland session, as
/// GNOME Shell's window list shows it. `false` when that cannot be read.
pub(crate) fn frame_on_xwayland(
    connection: &zbus::Connection,
    element: &AtspiElement,
    deadline: Deadline,
) -> bool {
    let Some(frame) = element.frame() else {
        return false;
    };
    frame_facts(connection, element, deadline)
        .is_some_and(|(title, _)| is_xwayland_frame(frame.pid(), &title, deadline))
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

/// Places one read from its window-coordinate rectangle: the screen rectangle
/// is that rectangle moved to `origin`, and the window rectangle stays the
/// evidence, tagged with its frame.
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

#[cfg(test)]
#[path = "shell_space_tests.rs"]
mod tests;
