//! What the front guard reads and does on an X11 desktop: the active window,
//! the stacking order and every client, a restack below the user's window, and
//! the activation that returns the focus. The window system answers, not the
//! accessibility bus, so an application that never reaches the bus is seen
//! too.

use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, FrontProcess, FrontView, FrontWindow, ProcessIdentity,
};

use crate::system::display::display_server::{DisplayServer, session_server};
use crate::system::process::{proc_stat, process_facts};
use crate::system::x11::control::activate::ACTIVATION_GRACE;
use crate::system::x11::control::control_send::{activate_window, restack_window_below};
use crate::system::x11::stacking::client_stack;
use crate::system::x11::stacking::stacking_source::StackingSource;
use crate::system::x11::{DisplayReader, X11Client, X11State};

const WRITE_BUDGET: Duration = ACTIVATION_GRACE;

pub(crate) fn read_front(deadline: Deadline) -> Result<FrontView, AdapterError> {
    let X11State::Ready { active, clients } = DisplayReader.clients_within(deadline) else {
        return Err(unreadable_or_unsupported());
    };
    if active.is_some_and(|id| clients.iter().all(|client| client.id != id)) {
        return Err(AdapterError::app_unresponsive("the active window"));
    }
    let stack = client_stack::read().unwrap_or_default();
    Ok(view_of(active, &clients, &stack))
}

/// The front view of a client list, the active window and the stacking order
/// from the bottom to the top.
pub(crate) fn view_of(active: Option<u32>, clients: &[X11Client], stack: &[u32]) -> FrontView {
    let window_of = |id: u32| {
        clients
            .iter()
            .find(|client| client.id == id)
            .map(|client| FrontWindow {
                id: id.to_string(),
                pid: client.pid.unwrap_or_default(),
            })
    };
    FrontView {
        focused: active.and_then(window_of),
        top: stack
            .iter()
            .rev()
            .filter(|id| {
                clients
                    .iter()
                    .any(|client| client.id == **id && !client.hidden)
            })
            .find_map(|id| window_of(*id)),
        windows: clients.iter().map(|client| client.id.to_string()).collect(),
        owners: clients.iter().filter_map(|client| client.pid).collect(),
    }
}

const USER_HZ: u64 = 100;

/// The executable name and the start time, in milliseconds since the Unix
/// epoch, of a process: its start in clock ticks after boot, from
/// `/proc/<pid>/stat`, added to the boot time in `/proc/stat`.
pub(crate) fn front_process(pid: u32) -> Option<FrontProcess> {
    let facts = process_facts::read(pid)?;
    let started = proc_stat::read_stat(pid).ok().flatten()?.start_time;
    Some(FrontProcess {
        image: facts.exe_basename.unwrap_or(facts.comm),
        started_ms: boot_seconds()? * 1_000 + started * 1_000 / USER_HZ,
    })
}

/// The running processes that descend from `root` and started at or after
/// `since_ms`, from the parent and start time in each `/proc/<pid>/stat`.
pub(crate) fn spawned_descendants(root: u32, since_ms: u64) -> Option<Vec<u32>> {
    let boot_ms = boot_seconds()? * 1_000;
    let mut parents = Vec::new();
    let mut starts = std::collections::HashMap::new();
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if let Ok(Some(stat)) = proc_stat::read_stat(pid) {
            parents.push((pid, stat.parent));
            starts.insert(pid, boot_ms + stat.start_time * 1_000 / USER_HZ);
        }
    }
    Some(agent_desktop_core::spawned_descendants_of(
        root,
        since_ms,
        &parents,
        |pid| starts.get(&pid).copied(),
    ))
}

fn boot_seconds() -> Option<u64> {
    std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("btime "))?
        .trim()
        .parse()
        .ok()
}

/// A session with an X11 display that did not answer is a failed read; any
/// other session has no front to read.
fn unreadable_or_unsupported() -> AdapterError {
    match session_server() {
        DisplayServer::X11(_) => AdapterError::app_unresponsive("the X11 display"),
        DisplayServer::Wayland | DisplayServer::Unavailable(_) => {
            AdapterError::not_supported("read_front")
        }
    }
}

fn client_of(window: &FrontWindow) -> Result<(u32, ProcessIdentity), AdapterError> {
    let id = window
        .id
        .parse()
        .map_err(|_| AdapterError::internal("front window id is not an X11 window"))?;
    let facts = process_facts::read(window.pid)
        .ok_or_else(|| AdapterError::stale_ref_because("The window's process has exited"))?;
    Ok((id, ProcessIdentity::new(window.pid, facts.instance)))
}

fn control_failed(operation: &str) -> AdapterError {
    AdapterError::new(
        agent_desktop_core::ErrorCode::ActionFailed,
        format!("The window manager did not take the {operation} request"),
    )
}

pub(crate) fn keep_below(window: &FrontWindow, above: &FrontWindow) -> Result<(), AdapterError> {
    let (id, owner) = client_of(window)?;
    let (sibling, _) = client_of(above)?;
    let deadline = Deadline::detached_after(WRITE_BUDGET.as_millis() as u64)?;
    restack_window_below(id, sibling, &owner, deadline).map_err(|_| control_failed("restack"))
}

pub(crate) fn give_back(window: &FrontWindow) -> Result<(), AdapterError> {
    let (id, owner) = client_of(window)?;
    let deadline = Deadline::detached_after(WRITE_BUDGET.as_millis() as u64)?;
    activate_window(id, &owner, deadline, true).map_err(|_| control_failed("activation"))
}

#[cfg(test)]
#[path = "focus_front_tests.rs"]
mod tests;
