//! Selection in a native combo box through its own window messages.
//!
//! A combo box that answers `GetComboBoxInfo` is the user32 combo box or a
//! superclass of it (WinForms). `CB_FINDSTRINGEXACT` finds the item by name,
//! `CB_SETCURSEL` selects it and `WM_COMMAND(CBN_SELCHANGE)` to the parent
//! runs the application's own change handler, with 0 foreground events and
//! the selection landing every time on Win32 and WinForms (C5). The dropdown
//! never opens. `CB_GETCURSEL` reads the selection back.

use agent_desktop_core::{AdapterError, Deadline};

use agent_desktop_core::DeliveryOutcome;

pub(crate) const COMBO_SELECT_LABEL: &str = "Win32.CB_SETCURSEL";

#[cfg(target_os = "windows")]
mod imp {
    use super::{AdapterError, Deadline, DeliveryOutcome};
    use crate::actions::native_message::{find_unique, send};
    use crate::tree::element::UIAElement;
    use std::ffi::c_void;
    use windows_sys::Win32::UI::Controls::{COMBOBOXINFO, GetComboBoxInfo};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CB_FINDSTRINGEXACT, CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE, GetDlgCtrlID, GetParent,
        PostMessageW, WM_COMMAND,
    };

    /// The element's window when it is a native combo box.
    pub(crate) fn native_combo(element: &UIAElement) -> Option<isize> {
        let hwnd: isize = element.0.get_native_window_handle().ok()?.into();
        if hwnd == 0 {
            return None;
        }
        let mut info = COMBOBOXINFO {
            cbSize: std::mem::size_of::<COMBOBOXINFO>() as u32,
            ..unsafe { std::mem::zeroed() }
        };
        (unsafe { GetComboBoxInfo(hwnd as *mut c_void, &mut info) } != 0).then_some(hwnd)
    }

    pub(crate) fn select_native_combo(
        hwnd: isize,
        value: &str,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let index = find_unique(hwnd, CB_FINDSTRINGEXACT, value, deadline)?;
        send(hwnd, CB_SETCURSEL, index, 0, deadline)?;
        let window = hwnd as *mut c_void;
        let id = (unsafe { GetDlgCtrlID(window) } as usize) & 0xFFFF;
        let notified = unsafe {
            PostMessageW(
                GetParent(window),
                WM_COMMAND,
                ((CBN_SELCHANGE as usize) << 16) | id,
                hwnd,
            )
        } != 0;
        let selected = send(hwnd, CB_GETCURSEL, 0, 0, deadline)
            .map_err(crate::actions::chain::after_delivery)?;
        Ok(DeliveryOutcome::from_delivery(
            true,
            notified && usize::try_from(selected).is_ok_and(|selected| selected == index),
        ))
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::{native_combo, select_native_combo};
