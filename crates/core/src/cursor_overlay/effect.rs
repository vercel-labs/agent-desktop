use crate::{Point, ProcessId, Rect};

/// An action's effect: where it plays, the element it outlines, whether it
/// ripples, and the process whose window holds the target, if any.
pub(crate) struct CursorEffect {
    pub(crate) destination: Point,
    pub(crate) target: Option<Rect>,
    pub(crate) click: bool,
    pub(crate) owner: Option<ProcessId>,
}
