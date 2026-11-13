use std::time::{Duration, Instant};

use agent_desktop_core::system::launch_options::LaunchOptions;
use agent_desktop_core::{AdapterError, Deadline, ErrorCode, WindowInfo};

use super::launch::launch_target_gone;
use super::launch_window::{LaunchWindow, exact_window, windowless};

const STARTUP_GRACE: Duration = Duration::from_millis(1500);

/// Waits only for the windows the launch itself produces. Starting up is what
/// creates them, so once the application reports that it finished starting up,
/// every window it was going to open on its own already exists and further
/// polling can only run out the deadline. An application that opens its first
/// window in response to being brought forward — TextEdit, Preview, any
/// document-based application — reports no window here, and `activate` is how a
/// caller asks for one instead of waiting for one it never requested.
pub(crate) fn settled_window(
    pid: i32,
    process_instance: &str,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    let mut poll_interval = Duration::from_millis(25);
    let mut grace_ends_at = None;
    let mut last_read_failure: Option<AdapterError>;
    let mut unplaced = None;
    let end = |unplaced: Option<LaunchWindow>, failure: Option<AdapterError>| match (
        unplaced.and_then(LaunchWindow::report),
        failure,
    ) {
        (Some(window), _) => Ok(Some(window)),
        (None, Some(error)) => Err(error),
        (None, None) => windowless(pid, process_instance),
    };
    loop {
        match exact_window(pid, process_instance, deadline) {
            Ok(LaunchWindow::Placed(window)) => return Ok(Some(window)),
            Ok(LaunchWindow::Unnamed) => return windowless(pid, process_instance),
            Ok(window @ LaunchWindow::Unplaced(_)) => {
                unplaced = Some(window);
                last_read_failure = None;
            }
            Ok(LaunchWindow::Absent) => last_read_failure = None,
            Err(error) if window_read_is_not_ready_yet(&error) => last_read_failure = Some(error),
            Err(error) => return Err(error),
        }
        if options.timeout_ms == 0
            || (!options.activate && grace_over(grace_ends_at, Instant::now()))
        {
            return end(unplaced, last_read_failure);
        }
        if grace_ends_at.is_none() && startup_finished(pid, process_instance)? {
            grace_ends_at = Instant::now().checked_add(STARTUP_GRACE);
        }
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return end(unplaced, last_read_failure);
        }
        std::thread::sleep(poll_interval.min(remaining));
        poll_interval = (poll_interval * 3 / 2).min(Duration::from_millis(250));
    }
}

fn window_read_is_not_ready_yet(error: &AdapterError) -> bool {
    matches!(error.code, ErrorCode::AppUnresponsive | ErrorCode::Timeout)
}

/// Ends the wait when the application has created whatever windows its launch
/// produces. The window server keeps no record of a process that exited, and
/// none of one that has not registered yet, so libproc decides which happened:
/// a process that is gone ends the launch with an error rather than an answer
/// about windows it will never open.
fn startup_finished(pid: i32, process_instance: &str) -> Result<bool, AdapterError> {
    use crate::system::appkit_bridge::StartupState;
    match crate::system::appkit_bridge::startup_state(pid) {
        StartupState::Starting => Ok(false),
        StartupState::Finished => Ok(true),
        StartupState::NoRecord => {
            if super::process::process_identity::matches_instance(pid, process_instance)? {
                Ok(true)
            } else {
                Err(launch_target_gone(pid))
            }
        }
    }
}

/// The grace covers the gap between an application reporting that it started
/// and its first window reaching the window server. Waiting starts running out
/// only once there is a completed startup to measure from.
fn grace_over(grace_ends_at: Option<Instant>, now: Instant) -> bool {
    grace_ends_at.is_some_and(|end| now >= end)
}

#[cfg(test)]
#[path = "launch_settled_tests.rs"]
mod tests;
