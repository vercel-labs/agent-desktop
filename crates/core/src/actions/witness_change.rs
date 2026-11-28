use serde::{Deserialize, Serialize};

/// One change between two witness reads around an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessChange {
    /// The target could not be read after the action: it was closed or replaced.
    TargetGone,
    /// The target's window is no longer one of its process's windows.
    WindowClosed,
    TargetSelected,
    TargetValue,
    SurfaceOpened,
    FocusIntoTarget,
    FocusMovedInWindow,
    FocusedValue,
    WindowTitle,
    WindowContent,
    WindowCount,
}

impl WitnessChange {
    /// A change that only the action explains. A window title, the window's
    /// content, the number of windows, the target's value and focus moving
    /// elsewhere in the window can also change by themselves (a timer, a
    /// playing scrubber, a notification), so they are weak evidence.
    pub fn is_strong(self) -> bool {
        matches!(
            self,
            Self::TargetGone
                | Self::WindowClosed
                | Self::TargetSelected
                | Self::SurfaceOpened
                | Self::FocusIntoTarget
        )
    }
}
