use agent_desktop_core::{AdapterError, Effect, InteractionPolicy, Rect, WindowFrameRecord};

use crate::system::window::window_frame_place::{FramePlace, same_frame};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WindowRestorePlan {
    Unminimize,
    SetFrame(Rect),
    AlreadyNormal,
    Unrecorded,
}

impl WindowRestorePlan {
    /// Decides the restore state from the window's minimized flag, its record and
    /// its frame. Nothing is read for a window that is not known to be visible.
    pub(crate) fn decide(
        minimized: Option<bool>,
        record: Option<&WindowFrameRecord>,
        current: impl FnOnce() -> Result<Rect, AdapterError>,
        work_area: impl FnOnce(Rect) -> Result<Rect, AdapterError>,
    ) -> Result<Self, AdapterError> {
        if minimized != Some(false) {
            return Ok(Self::Unminimize);
        }
        let frame = current()?;
        match record.map(|record| (record, FramePlace::of(record, frame))) {
            Some((record, FramePlace::Ours)) => Ok(Self::SetFrame(record.before)),
            Some((_, FramePlace::Before)) => Ok(Self::AlreadyNormal),
            Some((_, FramePlace::Elsewhere)) | None => {
                if same_frame(frame, work_area(frame)?) {
                    Ok(Self::Unrecorded)
                } else {
                    Ok(Self::AlreadyNormal)
                }
            }
        }
    }
}

impl WindowRestorePlan {
    /// Un-minimizing orders the window in front of the user's windows and no
    /// quiet primitive exists, so only a policy that lets a window be raised
    /// may carry it out. A frame move never raises and always passes.
    pub(crate) fn permitted_by(self, policy: InteractionPolicy) -> Result<Self, AdapterError> {
        if self == Self::Unminimize {
            policy.require(Effect::FocusSteal, "restore of a minimized window")?;
        }
        Ok(self)
    }
}

#[cfg(test)]
#[path = "window_restore_plan_tests.rs"]
mod tests;
