use agent_desktop_core::{Rect, WindowFrameRecord};

use crate::system::window::window_postcondition::TOLERANCE;

/// Where a window's frame stands against the record of a maximize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FramePlace {
    Before,
    Ours,
    Elsewhere,
}

impl FramePlace {
    /// Origin and size are judged apart. A frame is ours when each part still
    /// holds a value that our own move or resize wrote.
    pub(crate) fn of(record: &WindowFrameRecord, frame: Rect) -> Self {
        if same_frame(frame, record.before) {
            return Self::Before;
        }
        let origin_is_ours =
            same_origin(frame, record.before) || same_origin(frame, record.maximized);
        let size_is_ours = same_size(frame, record.before) || same_size(frame, record.maximized);
        if origin_is_ours && size_is_ours {
            Self::Ours
        } else {
            Self::Elsewhere
        }
    }
}

/// The record to keep after a maximize left the window at `frame`. Only a
/// frame the record does not explain replaces the maximized frame, as a window
/// that clamps its size leaves; a frame that is still ours, such as a move
/// whose resize has not landed, keeps the record as it is.
pub(crate) fn refreshed(record: WindowFrameRecord, frame: Rect) -> Option<WindowFrameRecord> {
    (FramePlace::of(&record, frame) == FramePlace::Elsewhere).then_some(WindowFrameRecord {
        maximized: frame,
        ..record
    })
}

pub(crate) fn same_frame(left: Rect, right: Rect) -> bool {
    same_origin(left, right) && same_size(left, right)
}

fn same_origin(left: Rect, right: Rect) -> bool {
    close(left.x, right.x) && close(left.y, right.y)
}

fn same_size(left: Rect, right: Rect) -> bool {
    close(left.width, right.width) && close(left.height, right.height)
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= TOLERANCE
}

#[cfg(test)]
#[path = "window_frame_place_tests.rs"]
mod tests;
