use crate::ref_action::post_release::PostRelease;
use crate::{ActionResult, NativeHandle};

/// A ref action as the dispatch left it, at the acknowledgement: the result,
/// the element it landed on, and the reads the caller makes after it has
/// released the interaction lease.
pub(crate) struct DispatchedAction {
    pub(crate) result: ActionResult,
    pub(crate) handle: NativeHandle,
    pub(crate) after_release: PostRelease,
}
