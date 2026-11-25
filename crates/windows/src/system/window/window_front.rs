//! What the front guard reads and does on Windows: the foreground window, the
//! z-order from `EnumWindows` (top first), a placement directly below the
//! user's window, and the foreground return.

use agent_desktop_core::{
    AdapterError, Deadline, FrontView, FrontWindow, InteractionLease, WindowInfo, WindowState,
};

use super::window_enum::{EnumeratedWindow, WindowHandle};
use super::window_ops::window_id;

const GIVE_BACK_BUDGET_MS: u64 = 1_500;

/// Whether an enumerated window can be the one the user sees in front: shown,
/// not cloaked, not minimized, not a tool window, and not empty.
pub(crate) fn can_be_front(window: &EnumeratedWindow) -> bool {
    window.visible && !window.cloaked && !window.iconic && !window.tool && !window.is_zero_sized()
}

/// One cheap pass over the z-order: the foreground window, the top window and
/// the handles of every shown window. The guard calls this every few
/// milliseconds, so nothing is read about a window except `IsWindowVisible`
/// until the first candidate for the top, and the pass reads only that
/// candidate's cloak, style and rectangle, in the order of the z-order, until
/// one qualifies.
#[cfg(target_os = "windows")]
pub(crate) fn read_front(_deadline: Deadline) -> Result<FrontView, AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindowVisible};

    let mut windows = Vec::new();
    let mut top = None;
    super::window_enum::enumerate_handles(|handle| {
        if unsafe { IsWindowVisible(handle) } == 0 {
            return true;
        }
        windows.push(window_id(handle));
        if top.is_none() && can_be_front(&super::window_enum::describe(handle)) {
            top = Some(handle);
        }
        true
    });
    let foreground = unsafe { GetForegroundWindow() };
    let front_window = |handle: WindowHandle| {
        let pid = super::frame_identity::application_pid(handle)?;
        Some(FrontWindow {
            id: window_id(handle),
            pid: pid.get(),
        })
    };
    if !foreground.is_null() && !windows.contains(&window_id(foreground)) {
        windows.push(window_id(foreground));
    }
    Ok(FrontView {
        focused: (!foreground.is_null())
            .then(|| front_window(foreground))
            .flatten(),
        top: top.and_then(front_window),
        windows,
        owners: Vec::new(),
    })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn read_front(_deadline: Deadline) -> Result<FrontView, AdapterError> {
    Err(AdapterError::not_supported("read_front"))
}

/// The running processes that descend from `root` and started at or after
/// `since_ms`, from the Toolhelp snapshot's parent ids. Only a descendant's
/// start time is read.
pub(crate) fn spawned_descendants(root: u32, since_ms: u64) -> Option<Vec<u32>> {
    let rows = crate::system::app_ops::process_snapshot().ok()?;
    let parents: Vec<(u32, u32)> = rows
        .iter()
        .map(|row| (row.pid.get(), row.parent.get()))
        .collect();
    Some(agent_desktop_core::spawned_descendants_of(
        root,
        since_ms,
        &parents,
        |pid| {
            crate::system::process_identity::ProcessIdentity::capture(
                agent_desktop_core::ProcessId::new(pid),
            )
            .ok()
            .flatten()
            .map(|identity| identity.started_unix_ms())
        },
    ))
}

/// The executable name and start time of the process behind a window.
pub(crate) fn front_process(pid: u32) -> Option<agent_desktop_core::FrontProcess> {
    let pid = agent_desktop_core::ProcessId::new(pid);
    let started = crate::system::process_identity::ProcessIdentity::capture(pid)
        .ok()
        .flatten()?;
    Some(agent_desktop_core::FrontProcess {
        image: crate::system::process_identity::process_image_name(pid)?,
        started_ms: started.started_unix_ms(),
    })
}

fn handle_of(window: &FrontWindow) -> WindowHandle {
    super::window_ops::parse_handle(&window.id)
}

/// Places `window` directly behind `above` without activating it.
#[cfg(target_os = "windows")]
pub(crate) fn keep_below(window: &FrontWindow, above: &FrontWindow) -> Result<(), AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };

    let (handle, anchor) = (handle_of(window), handle_of(above));
    if handle.is_null() || anchor.is_null() {
        return Err(AdapterError::internal("front window id is not an HWND"));
    }
    let ok = unsafe {
        SetWindowPos(
            handle,
            anchor,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        )
    };
    if ok == 0 {
        return Err(super::super::runtime::hresult::win32_last_error(
            "SetWindowPos below the user's window failed",
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn keep_below(_window: &FrontWindow, _above: &FrontWindow) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("keep_below"))
}

/// Returns the foreground to the user's window through the route
/// `focus-window` uses. The window is resolved from its handle and its
/// process, not found through a full window listing, which costs far more
/// than the return itself.
pub(crate) fn give_back(
    window: &FrontWindow,
    lease: &InteractionLease,
) -> Result<(), AdapterError> {
    let budget = Deadline::detached_after(GIVE_BACK_BUDGET_MS)?;
    let pid = agent_desktop_core::ProcessId::new(window.pid);
    let stored = WindowInfo {
        id: window.id.clone(),
        title: String::new(),
        app: String::new(),
        pid,
        process_instance: crate::system::process_identity::token_for_pid(pid)?,
        bounds: None,
        state: WindowState::default(),
    };
    let target = super::window_resolve::resolve_window_strict(&stored, budget)?;
    super::window_activate::focus_window(&target, &lease.with_deadline(budget)).map(drop)
}

#[cfg(test)]
#[path = "window_front_tests.rs"]
mod tests;
