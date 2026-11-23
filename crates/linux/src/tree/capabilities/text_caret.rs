//! Where a text node's insertion point sits, in the coordinates the node
//! reports its own extents in. The character at the caret offset marks it;
//! at the end of the text, where toolkits report no character, the edge of
//! the character before it does, or the start of the next line after a line
//! break.
use agent_desktop_core::{AdapterError, Deadline};
use zbus::Connection;

use super::text_selection::{BusText, TEXT_INTERFACE};
use crate::tree::placement::visibility::is_sentinel;
use crate::tree::{AtspiElement, Extents};

/// The caret's drawn width, the width the Windows overlay draws.
const CARET_WIDTH: i32 = 2;
/// The shortest caret, so a line a toolkit reports as very short still shows.
const MIN_CARET_HEIGHT: i32 = 10;

/// The AT-SPI coordinate types a caret is read in. A toolkit may support only
/// one: GTK 4 answers window coordinates only, and Chromium's window
/// coordinates for characters are not those of its own extents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordSpace {
    Screen,
    Window,
}

impl CoordSpace {
    const fn atspi(self) -> u32 {
        match self {
            Self::Screen => 0,
            Self::Window => 1,
        }
    }
}

/// What the caret read needs from a text node. `Ok(None)` is a read that
/// failed on its own, such as a coordinate type the toolkit does not support;
/// `Err` is the node or its application gone, or the deadline spent.
pub(crate) trait CaretSource {
    fn caret_offset(&mut self) -> Result<Option<i32>, AdapterError>;
    fn character_extents(
        &mut self,
        offset: i32,
        space: CoordSpace,
    ) -> Result<Option<Extents>, AdapterError>;
    /// Whether the character at `offset` ends a line. A secure field answers
    /// `false` without reading its text.
    fn ends_line(&mut self, offset: i32) -> Result<bool, AdapterError>;
}

fn usable(rect: Extents) -> bool {
    rect.height > 0 && !is_sentinel(rect)
}

fn caret_at(x: i32, y: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width: CARET_WIDTH,
        height: height.max(MIN_CARET_HEIGHT),
    }
}

/// The caret in `space`, or `None` when the node does not place one there.
pub(crate) fn caret_in<S: CaretSource>(
    source: &mut S,
    space: CoordSpace,
) -> Result<Option<Extents>, AdapterError> {
    let Some(offset) = source.caret_offset()?.filter(|offset| *offset >= 0) else {
        return Ok(None);
    };
    let Some(at) = source.character_extents(offset, space)? else {
        return Ok(None);
    };
    if usable(at) {
        return Ok(Some(caret_at(at.x, at.y, at.height)));
    }
    if offset == 0 {
        return Ok(None);
    }
    let Some(before) = source
        .character_extents(offset - 1, space)?
        .filter(|rect| usable(*rect))
    else {
        return Ok(None);
    };
    if !source.ends_line(offset - 1)? {
        return Ok(Some(caret_at(
            before.x.saturating_add(before.width),
            before.y,
            before.height,
        )));
    }
    let Some(line_start) = source
        .character_extents(0, space)?
        .filter(|rect| usable(*rect))
    else {
        return Ok(None);
    };
    Ok(Some(caret_at(
        line_start.x,
        before.y.saturating_add(before.height),
        before.height,
    )))
}

/// The caret in the first coordinate type the node places it in, screen
/// before window, with the type it was read in.
pub(crate) fn locate_caret<S: CaretSource>(
    source: &mut S,
) -> Result<Option<(Extents, CoordSpace)>, AdapterError> {
    for space in [CoordSpace::Screen, CoordSpace::Window] {
        if let Some(caret) = caret_in(source, space)? {
            return Ok(Some((caret, space)));
        }
    }
    Ok(None)
}

struct BusCaret<'a> {
    text: BusText<'a>,
    secure: bool,
}

impl CaretSource for BusCaret<'_> {
    fn caret_offset(&mut self) -> Result<Option<i32>, AdapterError> {
        self.text.integer("CaretOffset")
    }

    fn character_extents(
        &mut self,
        offset: i32,
        space: CoordSpace,
    ) -> Result<Option<Extents>, AdapterError> {
        Ok(self
            .text
            .call_text::<(i32, i32, i32, i32)>("GetCharacterExtents", (offset, space.atspi()))?
            .map(|(x, y, width, height)| Extents {
                x,
                y,
                width,
                height,
            }))
    }

    fn ends_line(&mut self, offset: i32) -> Result<bool, AdapterError> {
        if self.secure {
            return Ok(false);
        }
        let end = offset.saturating_add(1);
        Ok(self
            .text
            .call_text::<String>("GetText", (offset, end))?
            .is_some_and(|text| matches!(text.as_str(), "\n" | "\r" | "\u{2028}" | "\u{2029}")))
    }
}

/// The caret of a node that names the `Text` interface. A secure field is
/// placed without any of its text read.
pub(crate) fn read_caret(
    connection: &Connection,
    element: &AtspiElement,
    interfaces: Option<&[String]>,
    secure: bool,
    deadline: Deadline,
) -> Result<Option<(Extents, CoordSpace)>, AdapterError> {
    if !interfaces.is_some_and(|names| names.iter().any(|name| name == TEXT_INTERFACE)) {
        return Ok(None);
    }
    locate_caret(&mut BusCaret {
        text: BusText {
            connection,
            element,
            deadline,
        },
        secure,
    })
}

#[cfg(test)]
#[path = "text_caret_tests.rs"]
mod tests;
