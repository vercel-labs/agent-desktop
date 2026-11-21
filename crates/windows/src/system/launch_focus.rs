//! Keeps a launch without `--activate` from leaving the launched application
//! in the foreground.
//!
//! Two lines of defence. The process starts with a show-window hint that asks
//! for no activation; an application that ignores the hint and activates
//! itself is then given the foreground back, but only when the foreground
//! window belongs to the launched process or to a process it started, and the
//! window that had it before the spawn is still open. A window the user
//! switched to is never touched.

use std::time::{Duration, Instant};

use agent_desktop_core::{Deadline, InteractionLease, ProcessId, WindowFilter};

use super::app_ops::{ProcessRow, process_snapshot};
use super::process_identity::ProcessIdentity;
use super::window::window_activate::focus_window;
use super::window::window_ops::{list_windows_live, window_id};

const STARTF_USESHOWWINDOW: u32 = 0x1;
const SW_SHOWNOACTIVATE: u16 = 4;
const FOCUS_WAIT: Duration = Duration::from_millis(3_000);
const SETTLE_GRACE: Duration = Duration::from_millis(150);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const RESTORE_BUDGET_MS: u64 = 2_000;

/// The window that owns the foreground and the application process behind it:
/// the hosted application for a frame-hosted window, as window listing says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Foreground {
    pub handle: isize,
    pub pid: u32,
}

/// The `dwFlags` and `wShowWindow` fields of the child's `STARTUPINFOW`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StartupShow {
    pub flags: u32,
    pub show: u16,
}

#[derive(Debug, PartialEq, Eq)]
enum Look {
    Stolen,
    Waiting,
    Elsewhere,
}

pub(super) fn startup_show(activate: bool) -> StartupShow {
    if activate {
        return StartupShow { flags: 0, show: 0 };
    }
    StartupShow {
        flags: STARTF_USESHOWWINDOW,
        show: SW_SHOWNOACTIVATE,
    }
}

fn judge(prior: Foreground, now: Option<Foreground>, belongs: impl Fn(u32) -> bool) -> Look {
    match now {
        Some(now) if now.handle == prior.handle => Look::Waiting,
        Some(now) if belongs(now.pid) => Look::Stolen,
        _ => Look::Elsewhere,
    }
}

/// True when `pid` is `launched` or was started by it, directly or through
/// other processes that all pass `not_older`. Follows at most `rows.len() + 1`
/// links, so a parent cycle left by reused pids ends with false.
fn started_by(
    launched: u32,
    pid: u32,
    rows: &[ProcessRow],
    not_older: impl Fn(u32) -> bool,
) -> bool {
    let mut current = pid;
    for _ in 0..=rows.len() {
        if current == launched {
            return true;
        }
        let Some(row) = rows.iter().find(|row| row.pid.get() == current) else {
            return false;
        };
        if !not_older(current) {
            return false;
        }
        current = row.parent;
    }
    false
}

/// Whether `pid` belongs to the launch: the launched process, or a process it
/// started through processes that are all no older than it. A parent pid
/// outlives its process, so an older process whose parent's pid the launch
/// reused is never counted, and neither is one whose start time cannot be
/// read. `launch` is none when the launched process had already exited.
fn launch_owns(launched: ProcessId, launch: Option<ProcessIdentity>, pid: u32) -> bool {
    if pid == launched.get() {
        return true;
    }
    let Some(launch) = launch else {
        return false;
    };
    let rows = process_snapshot().unwrap_or_default();
    started_by(launched.get(), pid, &rows, |pid| {
        ProcessIdentity::capture(ProcessId::new(pid))
            .ok()
            .flatten()
            .is_some_and(|process| !process.started_before(launch))
    })
}

/// Waits for the launched process to take the foreground and, when it does,
/// focuses the window that had it. The wait ends when the foreground is
/// someone else's, when the launched window has been open for the grace period
/// without taking it, or after `FOCUS_WAIT`. True only when the previous
/// window reads back as the foreground afterwards.
pub(super) fn give_back(
    prior: Foreground,
    launched: ProcessId,
    lease: &InteractionLease,
    window_is_open: impl Fn() -> bool,
) -> bool {
    let wait = lease.deadline().capped(FOCUS_WAIT);
    let launch = ProcessIdentity::capture(launched).ok().flatten();
    let mut opened: Option<Instant> = None;
    loop {
        match judge(prior, read_foreground(), |pid| {
            launch_owns(launched, launch, pid)
        }) {
            Look::Stolen => return restore(prior, lease),
            Look::Elsewhere => return false,
            Look::Waiting => {}
        }
        if opened.is_none() && window_is_open() {
            opened = Some(Instant::now());
        }
        if opened.is_some_and(|at| at.elapsed() >= SETTLE_GRACE) || wait.is_expired() {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL.min(wait.remaining()));
    }
}

/// Focuses the window that had the foreground when a window of `owner` has
/// taken it since, as the prompt of an application asked to close can. A
/// foreground that did not change, or that went to another process, is left
/// alone.
pub(super) fn return_if_taken(
    prior: Foreground,
    owner: ProcessId,
    lease: &InteractionLease,
) -> bool {
    match judge(prior, read_foreground(), |pid| pid == owner.get()) {
        Look::Stolen => restore(prior, lease),
        Look::Waiting | Look::Elsewhere => false,
    }
}

fn restore(prior: Foreground, lease: &InteractionLease) -> bool {
    let Ok(budget) = Deadline::after(RESTORE_BUDGET_MS) else {
        return false;
    };
    let lease = lease.with_deadline(budget);
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };
    let Ok(windows) = list_windows_live(&filter, budget) else {
        return false;
    };
    let id = window_id(prior.handle as _);
    let Some(window) = windows
        .iter()
        .find(|window| window.id == id && window.pid == prior.pid)
    else {
        return false;
    };
    focus_window(window, &lease).is_ok() && read_foreground() == Some(prior)
}

#[cfg(target_os = "windows")]
pub(super) fn read_foreground() -> Option<Foreground> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let handle = unsafe { GetForegroundWindow() };
    if handle.is_null() {
        return None;
    }
    let pid = crate::system::window::frame_identity::application_pid(handle)?;
    Some(Foreground {
        handle: handle as isize,
        pid: pid.get(),
    })
}

#[cfg(not(target_os = "windows"))]
pub(super) fn read_foreground() -> Option<Foreground> {
    None
}

#[cfg(test)]
#[path = "launch_focus_tests.rs"]
mod tests;
