//! Popovers: the transient popup windows an application owns, such as a
//! ribbon drop-down or a split-button list.
//!
//! A popover is a visible popup window that another window of the same
//! process owns and that sits in the tool-window layer, so the window census
//! drops it. A menu has its own detector, and a tooltip never takes the
//! keyboard, so neither is a popover. Control type and ownership decide it,
//! never the application or the window title.

use agent_desktop_core::ProcessId;

use crate::system::window::window_enum::{EnumeratedWindow, WindowHandle};

/// What the window styles and the owner chain say about one tool window.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PopupFacts {
    pub(crate) owned_by_same_process: bool,
    pub(crate) popup_style: bool,
    pub(crate) no_activate: bool,
    pub(crate) menu_class: bool,
}

/// Whether a visible tool window is a popover.
pub(crate) fn is_popover(facts: &PopupFacts) -> bool {
    facts.owned_by_same_process && facts.popup_style && !facts.no_activate && !facts.menu_class
}

/// The popover candidates among a process's windows: visible, sized, not
/// cloaked tool windows whose facts say popover.
pub(crate) fn is_popover_window(window: &EnumeratedWindow, pid: ProcessId) -> bool {
    window.visible
        && window.tool
        && !window.is_zero_sized()
        && !window.cloaked
        && is_popover(&popup_facts(window.handle, pid))
}

#[cfg(target_os = "windows")]
fn popup_facts(handle: WindowHandle, pid: ProcessId) -> PopupFacts {
    use crate::system::window::window_identity::live_window_owner;
    use crate::system::window::window_ops::window_class_name;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetWindow, GetWindowLongW, WS_EX_NOACTIVATE, WS_POPUP,
    };
    let owner = unsafe { GetWindow(handle, GW_OWNER) };
    let style = unsafe { GetWindowLongW(handle, GWL_STYLE) } as u32;
    let ex_style = unsafe { GetWindowLongW(handle, GWL_EXSTYLE) } as u32;
    PopupFacts {
        owned_by_same_process: !owner.is_null() && live_window_owner(owner) == Some(pid),
        popup_style: style & WS_POPUP != 0,
        no_activate: ex_style & WS_EX_NOACTIVATE != 0,
        menu_class: window_class_name(handle).as_deref() == Some("#32768"),
    }
}

#[cfg(not(target_os = "windows"))]
fn popup_facts(_handle: WindowHandle, _pid: ProcessId) -> PopupFacts {
    PopupFacts {
        owned_by_same_process: false,
        popup_style: false,
        no_activate: false,
        menu_class: false,
    }
}

#[cfg(test)]
#[path = "popover_inventory_tests.rs"]
mod tests;
