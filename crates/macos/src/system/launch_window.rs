//! The window a launch reports for the process it launched or attached to.

use agent_desktop_core::{AdapterError, ErrorCode, WindowInfo};

/// What one exact read of a launched process says about its window.
pub(crate) enum LaunchWindow {
    /// The process has no window yet.
    Absent,
    /// The process has several windows and none is focused, so there is no
    /// one window to report.
    Unnamed,
    /// One window whose bounds the window server has not set yet.
    Unplaced(WindowInfo),
    /// One window with its bounds set.
    Placed(WindowInfo),
}

impl LaunchWindow {
    /// Judges one exact read. A read of another process instance is an error.
    pub(crate) fn of_read(
        read: Result<WindowInfo, AdapterError>,
        pid: i32,
        process_instance: &str,
    ) -> Result<Self, AdapterError> {
        let window = match read {
            Ok(window) => window,
            Err(error) if error.code == ErrorCode::WindowNotFound => return Ok(Self::Absent),
            Err(error) if error.code == ErrorCode::AmbiguousTarget => return Ok(Self::Unnamed),
            Err(error) => return Err(error),
        };
        if window.process_instance.as_deref() != Some(process_instance) {
            return Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "Application process instance changed while waiting for its window",
            )
            .with_details(serde_json::json!({ "pid": pid, "complete": false })));
        }
        let placed = window
            .bounds
            .is_some_and(|bounds| bounds.width > 0.0 && bounds.height > 0.0);
        Ok(if placed {
            Self::Placed(window)
        } else {
            Self::Unplaced(window)
        })
    }

    /// The window to report when the wait ends now: a placed window, or an
    /// unplaced one without bounds rather than with bounds that are not real.
    pub(crate) fn report(self) -> Option<WindowInfo> {
        match self {
            Self::Placed(window) => Some(window),
            Self::Unplaced(mut window) => {
                window.bounds = None;
                Some(window)
            }
            Self::Absent | Self::Unnamed => None,
        }
    }
}

/// One exact read of the launched process's window.
#[cfg(target_os = "macos")]
pub(super) fn exact_window(
    pid: i32,
    process_instance: &str,
    deadline: agent_desktop_core::Deadline,
) -> Result<LaunchWindow, AdapterError> {
    LaunchWindow::of_read(
        crate::system::window::window_inventory::exact_window_for_pid_until(pid, deadline),
        pid,
        process_instance,
    )
}

/// The windowless answer of a launch, true only while the launched process
/// instance still runs.
#[cfg(target_os = "macos")]
pub(super) fn windowless(
    pid: i32,
    process_instance: &str,
) -> Result<Option<WindowInfo>, AdapterError> {
    if super::process::process_identity::matches_instance(pid, process_instance)? {
        Ok(None)
    } else {
        Err(super::launch::launch_target_gone(pid))
    }
}

#[cfg(test)]
#[path = "launch_window_tests.rs"]
mod tests;
