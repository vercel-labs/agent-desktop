//! Per-process surface inventory test, driven against the live desktop.
#![cfg(all(test, target_os = "windows"))]

use super::list_surfaces_for_process;
use crate::system::app_ops::process_snapshot;
use crate::system::process_identity::token_for_pid;
use crate::system::window::window_enum::enumerate_top_level;
use crate::system::window::window_identity::live_window_owner;
use crate::system::window::window_ops::passes_filter;
use crate::tree::walk::walker_fake::deadline;
use agent_desktop_core::{ProcessId, ProcessIdentity};

fn identity_for(pid: u32) -> ProcessIdentity {
    let token = token_for_pid(ProcessId::from(pid))
        .expect("the process table is readable")
        .unwrap_or_else(|| panic!("pid {pid} carries a generation token"));
    ProcessIdentity::new(ProcessId::from(pid), token)
}

/// A process that presents no agent-facing window answers an empty list and
/// `ok` - a statement about that process, never `PLATFORM_NOT_SUPPORTED` and
/// never an error. The process is found live, by name, and its windowless
/// premise is verified against the desktop's own enumeration before the
/// assertion is made.
#[test]
fn a_process_with_no_windows_returns_an_empty_inventory() {
    let pid = windowless_process_pid().expect("a live windowless process exists on this desktop");

    let surfaces = list_surfaces_for_process(identity_for(pid), deadline())
        .expect("a windowless process inventories successfully");

    assert!(
        surfaces.is_empty(),
        "a process with no agent-facing windows presents no surfaces: {surfaces:?}"
    );
}

/// Finds a live process that owns no agent-facing top-level window: a
/// background-system image name from the live process table whose pid owns
/// nothing the census filter would admit, and whose generation token is
/// readable so the inventory's identity check can be satisfied.
fn windowless_process_pid() -> Option<u32> {
    const WINDOWLESS_NAMES: [&str; 5] = [
        "svchost.exe",
        "conhost.exe",
        "fontdrvhost.exe",
        "spoolsv.exe",
        "services.exe",
    ];
    let mut window_owners = std::collections::HashSet::new();
    enumerate_top_level(|window| {
        if let Some(owner) = live_window_owner(window.handle) {
            if passes_filter(&window) {
                window_owners.insert(u32::from(owner));
            }
        }
        true
    })
    .expect("the desktop's top-level windows enumerate");
    process_snapshot()
        .ok()?
        .into_iter()
        .find(|row| {
            WINDOWLESS_NAMES.contains(&row.name.to_ascii_lowercase().as_str())
                && !window_owners.contains(&u32::from(row.pid))
        })
        .and_then(|row| {
            token_for_pid(row.pid)
                .ok()
                .flatten()
                .map(|_| u32::from(row.pid))
        })
}
