//! The typing caret of a text node on screen. A character rectangle is read
//! in the first coordinate type the toolkit places it in; its offset from the
//! node's own rectangle in that same type is added to the rectangle the node
//! is placed from, and the sum lands on the stage the way the node does. A
//! toolkit whose screen and window coordinates disagree (Chromium), one that
//! answers window coordinates only (GTK 4), and an Xwayland toolkit whose
//! coordinates are scaled place the caret the same way.
use agent_desktop_core::{AdapterError, Deadline, NativeHandle, ProcessId, Rect};

use super::caret_route::CaretRoute;
use super::focused_node::focused_element;
use super::live_project::secure;
use super::{BusReader, NodeReader, read_live, with_reader};
use crate::system::SharedBus;
use crate::system::display::display_server::{DisplayServer, session_server};
use crate::tree::capabilities::text_caret::{CoordSpace, drawn_caret, read_caret};
use crate::tree::placement::shell_place::ShellPlace;
use crate::tree::placement::visibility::is_sentinel;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::vocabulary::role_numbers::PASSWORD_TEXT;
use crate::tree::{Extents, atspi_element};

/// The node's rectangle in each coordinate type a caret can be read in, and
/// how its frame lands on the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CaretFrame {
    pub(crate) screen: Option<Extents>,
    pub(crate) window: Option<Extents>,
    pub(crate) place: ShellPlace,
}

/// `caret`, read in `space`, moved onto the node's stage rectangle.
pub(crate) fn caret_on_screen(
    caret: Extents,
    space: CoordSpace,
    frame: CaretFrame,
) -> Option<Rect> {
    let reference = match space {
        CoordSpace::Screen => frame.screen,
        CoordSpace::Window => frame.window,
    }
    .filter(|rect| !is_sentinel(*rect))?;
    let source = frame.place.source(frame.screen, frame.window)?;
    frame
        .place
        .to_stage(Extents {
            x: caret.x.checked_sub(reference.x)?.checked_add(source.x)?,
            y: caret.y.checked_sub(reference.y)?.checked_add(source.y)?,
            width: caret.width,
            height: caret.height,
        })
        .map(|stage| Rect::from(drawn_caret(stage)))
}

fn caret_via(
    reader: &mut BusReader<'_>,
    handle: &NativeHandle,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    let server = session_server();
    if matches!(server, DisplayServer::X11(_)) {
        return CaretRoute::X11Session.place().map(|_| None);
    }
    let node = read_live(reader, handle)?;
    let element = atspi_element(handle)?;
    let placed = reader.placement(element, deadline);
    let connection = reader.connection;
    let place = CaretRoute::of(&server, placed.shell).place()?;
    let traits = &node.read.node.traits;
    let is_secure = traits.role.present() == Some(&PASSWORD_TEXT) || secure(&node.evidence);
    let interfaces = traits.interfaces.present().map(Vec::as_slice);
    let Some((caret, space)) = read_caret(connection, element, interfaces, is_secure, deadline)?
    else {
        return Ok(None);
    };
    Ok(caret_on_screen(
        caret,
        space,
        CaretFrame {
            screen: node.read.node.extents.present().copied(),
            window: placed.node,
            place,
        },
    ))
}

/// The screen rectangle of the insertion point of the text node `handle`
/// names. `None` when the node places no caret; refused off a window the
/// GNOME Shell extension placed.
pub(crate) fn get_insertion_caret(
    bus: &SharedBus,
    handle: &NativeHandle,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    with_reader(
        bus,
        handle,
        deadline,
        ReadPurpose::Bounds,
        |reader, handle| caret_via(reader, handle, deadline),
    )
}

/// [`get_insertion_caret`] for the node holding keyboard focus in `process`,
/// or anywhere on the desktop when `process` is `None`. `None` when nothing
/// there holds focus.
pub(crate) fn get_focused_insertion_caret(
    bus: &SharedBus,
    process: Option<ProcessId>,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    if matches!(session_server(), DisplayServer::X11(_)) {
        return CaretRoute::X11Session.place().map(|_| None);
    }
    let Some(element) = focused_element(bus, process.map(ProcessId::get), deadline)? else {
        return Ok(None);
    };
    get_insertion_caret(bus, &element.into_native_handle(), deadline)
}

#[cfg(test)]
#[path = "live_caret_tests.rs"]
mod tests;
