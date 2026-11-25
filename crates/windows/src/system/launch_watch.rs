use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ProcessId, WindowFilter, WindowInfo,
    system::launch_options::LaunchOptions,
};

use super::super::app_ops::{ProcessRow, process_snapshot};
use super::super::process_identity;
use super::super::window::window_ops::list_windows_live;
use super::after_launch;

const POLL_START: Duration = Duration::from_millis(50);
const POLL_LIMIT: Duration = Duration::from_millis(250);

/// Where the wait for a launch's window stands. The wait ends in exactly one
/// of three ways, and each way is a different honest answer for the caller:
/// a window was seen, the launched process and everything it started are gone
/// without one, or the wait ran out while the launch was still running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Watch {
    Polling,
    WindowSeen,
    LauncherFinished,
    TimedOut,
}

/// What one look at the desktop showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sight {
    Window,
    NoWindow,
    NothingLeftRunning,
    OutOfTime,
}

impl Watch {
    pub(super) fn advance(self, sight: Sight) -> Self {
        match (self, sight) {
            (Self::Polling, Sight::Window) => Self::WindowSeen,
            (Self::Polling, Sight::NothingLeftRunning) => Self::LauncherFinished,
            (Self::Polling, Sight::OutOfTime) => Self::TimedOut,
            (Self::Polling, Sight::NoWindow) => Self::Polling,
            (ended, _) => ended,
        }
    }
}

pub(super) struct Sighting {
    pub(super) window: Option<WindowInfo>,
    pub(super) end: Watch,
}

pub(super) fn should_poll_after_first_observation(timeout_ms: u64) -> bool {
    timeout_ms > 0
}

/// The processes the root started, directly or through other processes that
/// are still running. A launcher that hands its work to a child owns that
/// child's windows; a window of an unrelated process is never the launch's.
pub(super) fn descendants(rows: &[ProcessRow], root: ProcessId) -> Vec<ProcessId> {
    let mut found = vec![root];
    let mut index = 0;
    while index < found.len() {
        let parent = found[index];
        for row in rows {
            if row.parent == parent && !found.contains(&row.pid) {
                found.push(row.pid);
            }
        }
        index += 1;
    }
    found.remove(0);
    found
}

fn launch_finished(pid: ProcessId) -> bool {
    process_identity::wait_for_exit(pid, Duration::ZERO)
        && process_snapshot()
            .map(|rows| descendants(&rows, pid).is_empty())
            .unwrap_or(false)
}

/// Watches until the launch shows a window, finishes without one, or the
/// deadline passes. A window counts when the launched process owns it or a
/// process it started does.
pub(super) fn observe_window(
    pid: ProcessId,
    process_instance: &str,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<Sighting, AdapterError> {
    let polls = should_poll_after_first_observation(options.timeout_ms);
    let mut state = Watch::Polling;
    let mut interval = POLL_START;
    loop {
        let window = observe_window_once(pid, process_instance, deadline)?;
        let sight = if window.is_some() {
            Sight::Window
        } else if launch_finished(pid) {
            Sight::NothingLeftRunning
        } else if !polls || deadline.remaining().is_zero() {
            Sight::OutOfTime
        } else {
            Sight::NoWindow
        };
        state = state.advance(sight);
        if state != Watch::Polling {
            return Ok(Sighting { window, end: state });
        }
        std::thread::sleep(interval.min(deadline.remaining()));
        interval = (interval * 3 / 2).min(POLL_LIMIT);
    }
}

/// One window observation, tolerant of the inventory's mid-walk identity race.
///
/// `list_windows_live` absorbs that race itself, retrying inside the shared
/// listing budget before it returns `WINDOW_NOT_FOUND`. A race that survives
/// the budget, and a budget that runs out (`TIMEOUT`), both say the desktop
/// could not be read coherently in time, not that this launch failed, so both
/// are reported as no window observed. Every other failure propagates: a
/// target whose process instance changed underneath the launch is a real
/// answer, not a race.
pub(super) fn observe_window_once(
    pid: ProcessId,
    process_instance: &str,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    match exact_window(pid, process_instance, deadline) {
        Ok(window) => Ok(window),
        Err(error) if is_unobservable_within_budget(&error) => Ok(None),
        Err(error) => Err(after_launch(error)),
    }
}

fn is_unobservable_within_budget(error: &AdapterError) -> bool {
    matches!(error.code, ErrorCode::WindowNotFound | ErrorCode::Timeout)
}

fn exact_window(
    pid: ProcessId,
    process_instance: &str,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    let windows = list_windows_live(
        &WindowFilter {
            focused_only: false,
            app: None,
        },
        deadline,
    )?;
    if let Some(window) = windows.iter().find(|window| window.pid == pid) {
        if window.process_instance.as_deref() != Some(process_instance) {
            return Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "Application process instance changed while waiting for its window",
            )
            .with_details(serde_json::json!({ "pid": pid, "complete": false })));
        }
        return Ok(Some(window.clone()));
    }
    if windows.is_empty() {
        return Ok(None);
    }
    let started = process_snapshot()
        .map(|rows| descendants(&rows, pid))
        .unwrap_or_default();
    Ok(windows
        .into_iter()
        .find(|window| started.contains(&window.pid) && window.process_instance.is_some()))
}

#[cfg(test)]
#[path = "launch_watch_tests.rs"]
mod tests;
