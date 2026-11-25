//! What the front guard reads and does on Windows: the foreground window, the
//! z-order from `EnumWindows` (top first), a placement directly below the
//! user's window, and the foreground return.

use agent_desktop_core::{
    AdapterError, Deadline, FrontView, FrontWindow, InteractionLease, WindowFilter,
};

use super::window_enum::{EnumeratedWindow, WindowHandle, enumerate_top_level};
use super::window_ops::{list_windows_live, window_id};

const GIVE_BACK_BUDGET_MS: u64 = 1_500;

/// Whether an enumerated window can be the one the user sees in front: shown,
/// not cloaked, not minimized, not a tool window, and not empty.
fn can_be_front(window: &EnumeratedWindow) -> bool {
    window.visible && !window.cloaked && !window.iconic && !window.tool && !window.is_zero_sized()
}

/// The top window and every listed window, from a z-order that runs top to
/// bottom.
pub(crate) fn order_of(order: &[EnumeratedWindow]) -> (Option<usize>, Vec<usize>) {
    let top = order.iter().position(can_be_front);
    let listed = order
        .iter()
        .enumerate()
        .filter(|(_, window)| window.visible && !window.cloaked)
        .map(|(index, _)| index)
        .collect();
    (top, listed)
}

#[cfg(target_os = "windows")]
pub(crate) fn read_front(_deadline: Deadline) -> Result<FrontView, AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let mut order = Vec::new();
    enumerate_top_level(|window| {
        order.push(window);
        true
    })?;
    let (top, listed) = order_of(&order);
    let foreground = unsafe { GetForegroundWindow() };
    let front_window = |handle: WindowHandle| {
        let pid = super::frame_identity::application_pid(handle)?;
        Some(FrontWindow {
            id: window_id(handle),
            pid: pid.get(),
        })
    };
    let mut windows: Vec<String> = listed
        .iter()
        .map(|index| window_id(order[*index].handle))
        .collect();
    if !foreground.is_null() && !windows.contains(&window_id(foreground)) {
        windows.push(window_id(foreground));
    }
    Ok(FrontView {
        focused: (!foreground.is_null())
            .then(|| front_window(foreground))
            .flatten(),
        top: top.and_then(|index| front_window(order[index].handle)),
        windows,
    })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn read_front(_deadline: Deadline) -> Result<FrontView, AdapterError> {
    Err(AdapterError::not_supported("read_front"))
}

/// Puts a window directly below the user's foreground window without
/// activating it, so a window shown without activation lands in front of the
/// windows beneath the user's and not at the bottom of the order.
#[cfg(target_os = "windows")]
pub(crate) fn place_below_foreground(
    handle: super::window_enum::WindowHandle,
) -> Result<(), AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };

    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_null() || foreground == handle {
        return Ok(());
    }
    let ok = unsafe {
        SetWindowPos(
            handle,
            foreground,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        )
    };
    if ok == 0 {
        return Err(super::window_op::after_write(
            crate::system::runtime::hresult::win32_last_error(
                "SetWindowPos below the foreground window failed",
            ),
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn place_below_foreground(
    _handle: super::window_enum::WindowHandle,
) -> Result<(), AdapterError> {
    Ok(())
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

/// Returns the foreground to the user's window through the same route
/// `focus-window` uses.
pub(crate) fn give_back(
    window: &FrontWindow,
    lease: &InteractionLease,
) -> Result<(), AdapterError> {
    let budget = Deadline::detached_after(GIVE_BACK_BUDGET_MS)?;
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };
    let id = window.id.clone();
    let target = list_windows_live(&filter, budget)?
        .into_iter()
        .find(|candidate| candidate.id == id && candidate.pid.get() == window.pid)
        .ok_or_else(|| AdapterError::stale_ref_because("The user's window has closed"))?;
    super::window_activate::focus_window(&target, &lease.with_deadline(budget)).map(drop)
}

#[cfg(test)]
#[path = "window_front_tests.rs"]
mod tests;
