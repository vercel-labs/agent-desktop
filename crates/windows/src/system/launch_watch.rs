use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ProcessId, WindowFilter, WindowInfo,
    system::launch_options::LaunchOptions,
};

use super::super::app_ops::{ProcessRow, process_snapshot};
use super::super::process_identity;
use super::super::window::window_ops::list_windows_live;
use super::after_launch;
use super::launch_attach::{image_of, same_file};

const POLL_START: Duration = Duration::from_millis(50);
const POLL_LIMIT: Duration = Duration::from_millis(250);

/// How long a launch that may have been forwarded to a running instance keeps
/// looking for the window it opens after its own launcher is gone. A forward
/// opens its window in another process a moment after the launcher exits.
const FORWARD_GRACE: Duration = Duration::from_millis(3_000);

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

/// Whose window the launch reports. A launcher can hand its work to a process
/// it started, or to an instance of the same program that was already running
/// and opens the window itself; each is a different arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    Launched,
    Started,
    Resident,
}

pub(super) struct Seen {
    pub(super) window: WindowInfo,
    pub(super) origin: Origin,
}

pub(super) struct Sighting {
    pub(super) seen: Option<Seen>,
    pub(super) end: Watch,
}

/// What the launch looks for. `resident` is set when the launch can hand its
/// work to a running instance of the same program: the image of that program
/// and the windows that existed before the launch, so that only a window the
/// launch caused counts as its arrival.
pub(super) struct Target<'a> {
    pub(super) pid: ProcessId,
    pub(super) process_instance: &'a str,
    pub(super) resident: Option<Resident>,
}

pub(super) struct Resident {
    pub(super) image: PathBuf,
    pub(super) known_windows: HashSet<String>,
}

/// The ids of the windows open now, or `None` when the desktop could not be
/// read, in which case no later window can be called new.
pub(super) fn window_ids(deadline: Deadline) -> Option<HashSet<String>> {
    list_windows_live(&all_windows(), deadline)
        .ok()
        .map(|windows| windows.into_iter().map(|window| window.id).collect())
}

fn all_windows() -> WindowFilter {
    WindowFilter {
        focused_only: false,
        app: None,
    }
}

/// Whether the wait may end because the launcher is gone. A launch that can
/// be a forward keeps looking for the window of the instance it forwarded to
/// until the grace after the launcher's exit has passed.
pub(super) fn forward_search_over(forwardable: bool, finished: Instant, now: Instant) -> bool {
    !forwardable || now.saturating_duration_since(finished) >= FORWARD_GRACE
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
    target: &Target<'_>,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<Sighting, AdapterError> {
    let pid = target.pid;
    let polls = should_poll_after_first_observation(options.timeout_ms);
    let mut state = Watch::Polling;
    let mut interval = POLL_START;
    let mut finished_at: Option<Instant> = None;
    loop {
        let seen = observe_window_once(target, deadline)?;
        let sight = if seen.is_some() {
            Sight::Window
        } else if launch_finished(pid)
            && forward_search_over(
                target.resident.is_some(),
                *finished_at.get_or_insert_with(Instant::now),
                Instant::now(),
            )
        {
            Sight::NothingLeftRunning
        } else if !polls || deadline.remaining().is_zero() {
            Sight::OutOfTime
        } else {
            Sight::NoWindow
        };
        state = state.advance(sight);
        if state != Watch::Polling {
            return Ok(Sighting { seen, end: state });
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
    target: &Target<'_>,
    deadline: Deadline,
) -> Result<Option<Seen>, AdapterError> {
    match exact_window(target, deadline) {
        Ok(seen) => Ok(seen),
        Err(error) if is_unobservable_within_budget(&error) => Ok(None),
        Err(error) => Err(after_launch(error)),
    }
}

fn is_unobservable_within_budget(error: &AdapterError) -> bool {
    matches!(error.code, ErrorCode::WindowNotFound | ErrorCode::Timeout)
}

fn exact_window(target: &Target<'_>, deadline: Deadline) -> Result<Option<Seen>, AdapterError> {
    let windows = list_windows_live(&all_windows(), deadline)?;
    if let Some(window) = windows.iter().find(|window| window.pid == target.pid) {
        if window.process_instance.as_deref() != Some(target.process_instance) {
            return Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "Application process instance changed while waiting for its window",
            )
            .with_details(serde_json::json!({ "pid": target.pid, "complete": false })));
        }
        return Ok(Some(Seen {
            window: window.clone(),
            origin: Origin::Launched,
        }));
    }
    if windows.is_empty() {
        return Ok(None);
    }
    let started = process_snapshot()
        .map(|rows| descendants(&rows, target.pid))
        .unwrap_or_default();
    if let Some(window) = windows
        .iter()
        .find(|window| started.contains(&window.pid) && window.process_instance.is_some())
    {
        return Ok(Some(Seen {
            window: window.clone(),
            origin: Origin::Started,
        }));
    }
    Ok(target
        .resident
        .as_ref()
        .and_then(|resident| new_window_of_image(windows, resident))
        .map(|window| Seen {
            window,
            origin: Origin::Resident,
        }))
}

/// A window that did not exist before the launch and belongs to a running
/// process of the launched program. A launcher that forwards to a single-instance
/// application leaves exactly this behind.
fn new_window_of_image(windows: Vec<WindowInfo>, resident: &Resident) -> Option<WindowInfo> {
    windows.into_iter().find(|window| {
        window.process_instance.is_some()
            && !resident.known_windows.contains(&window.id)
            && image_of(window.pid).is_some_and(|image| same_file(&image, &resident.image))
    })
}

#[cfg(test)]
#[path = "launch_watch_tests.rs"]
mod tests;
