use crate::{ActionResult, NativeHandle, PlatformAdapter, ProcessIdentity};

/// A ref action as the dispatch left it, at the acknowledgement: the result,
/// the element it landed on, and the process whose overlays the caller reads
/// after it has released the interaction lease. The overlay read is not part
/// of the hold: the lease covers resolving the target and dispatching to it.
pub(crate) struct DispatchedAction {
    pub(crate) result: ActionResult,
    pub(crate) handle: NativeHandle,
    pub(crate) surfaces_of: Option<ProcessIdentity>,
}

/// An action that opens a sheet, menu, or alert leaves the caller staring at a
/// success envelope with no hint that the application is now waiting on a
/// dialog. Reporting the overlays saves a round of window archaeology. Purely
/// informational, so a failed read never disturbs delivered work. It runs after
/// the interaction lease is released: an application that has just opened a
/// modal answers slowly, and no other agent should wait behind that.
pub(crate) fn read_surfaces_after_release(
    adapter: &dyn PlatformAdapter,
    surfaces_of: Option<ProcessIdentity>,
    raised_by: &NativeHandle,
    deadline: crate::Deadline,
    result: &mut ActionResult,
) {
    if let Some(process) = surfaces_of {
        result.surfaces = adapter
            .surfaces_raised_by(process, raised_by, deadline)
            .unwrap_or_default();
    }
}
