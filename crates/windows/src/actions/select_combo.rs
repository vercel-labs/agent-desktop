//! Selection in a native combo box through its own window messages.
//!
//! A combo box that answers `GetComboBoxInfo` is the user32 combo box or a
//! superclass of it (WinForms). `CB_FINDSTRINGEXACT` finds the item by name,
//! `CB_SETCURSEL` selects it and `WM_COMMAND(CBN_SELCHANGE)` to the parent
//! runs the application's own change handler, with 0 foreground events and
//! the selection landing every time on Win32 and WinForms (C5). The dropdown
//! never opens. The posted handler runs after the call returns and can still
//! change the selection, so the result is delivered and unverified.
//!
//! An owner-drawn combo without `CBS_HASSTRINGS` keeps item data, not names,
//! so the name search cannot see its items. Such a combo, and a name the search
//! misses, hand the selection back to the caller's own route instead of
//! failing, because nothing was sent yet.

use agent_desktop_core::{AdapterError, Deadline};

use agent_desktop_core::DeliveryOutcome;

pub(crate) const COMBO_SELECT_LABEL: &str = "Win32.CB_SETCURSEL";

const CBS_OWNERDRAWFIXED: u32 = 0x0010;
const CBS_OWNERDRAWVARIABLE: u32 = 0x0020;
const CBS_HASSTRINGS: u32 = 0x0200;

/// Whether a combo box with this window style stores item names. Only an
/// owner-drawn combo box can lack them, and it says so by omitting
/// `CBS_HASSTRINGS`.
pub(crate) fn combo_has_strings(style: u32) -> bool {
    style & (CBS_OWNERDRAWFIXED | CBS_OWNERDRAWVARIABLE) == 0 || style & CBS_HASSTRINGS != 0
}

/// The index a name search found, `None` when it found no item, so the
/// caller can try the selection route that needs no item names.
pub(crate) fn index_or_fall_through(
    found: Result<usize, AdapterError>,
) -> Result<Option<usize>, AdapterError> {
    match found {
        Ok(index) => Ok(Some(index)),
        Err(error) if error.code == agent_desktop_core::ErrorCode::ElementNotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        AdapterError, Deadline, DeliveryOutcome, combo_has_strings, index_or_fall_through,
    };
    use crate::actions::native_message::{find_unique, send};
    use crate::actions::native_window::NativeWindow;
    use crate::tree::element::UIAElement;
    use std::ffi::c_void;
    use windows_sys::Win32::UI::Controls::{COMBOBOXINFO, GetComboBoxInfo};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CB_FINDSTRINGEXACT, CB_SETCURSEL, CBN_SELCHANGE, GWL_STYLE, GetDlgCtrlID, GetParent,
        GetWindowLongPtrW, PostMessageW, WM_COMMAND,
    };

    /// The element's window when it is a native combo box of the element's
    /// verified process.
    pub(crate) fn native_combo(element: &UIAElement) -> Option<NativeWindow> {
        let hwnd: isize = element.0.get_native_window_handle().ok()?.into();
        let combo = NativeWindow::of(hwnd, element)?;
        let mut info = COMBOBOXINFO {
            cbSize: std::mem::size_of::<COMBOBOXINFO>() as u32,
            ..unsafe { std::mem::zeroed() }
        };
        (unsafe { GetComboBoxInfo(hwnd as *mut c_void, &mut info) } != 0).then_some(combo)
    }

    pub(crate) fn select_native_combo(
        combo: NativeWindow,
        value: &str,
        deadline: Deadline,
    ) -> Result<Option<DeliveryOutcome>, AdapterError> {
        let style = unsafe { GetWindowLongPtrW(combo.hwnd as *mut c_void, GWL_STYLE) } as u32;
        if !combo_has_strings(style) {
            return Ok(None);
        }
        let Some(index) =
            index_or_fall_through(find_unique(&combo, CB_FINDSTRINGEXACT, value, deadline))?
        else {
            return Ok(None);
        };
        send(&combo, CB_SETCURSEL, index, 0, deadline)?;
        let hwnd = combo.hwnd;
        let window = hwnd as *mut c_void;
        let id = (unsafe { GetDlgCtrlID(window) } as usize) & 0xFFFF;
        let parent = unsafe { GetParent(window) };
        if !parent.is_null() {
            combo
                .ensure_owned(parent as isize)
                .map_err(crate::actions::chain::after_delivery)?;
            unsafe {
                PostMessageW(
                    parent,
                    WM_COMMAND,
                    ((CBN_SELCHANGE as usize) << 16) | id,
                    hwnd,
                );
            }
        }
        Ok(Some(DeliveryOutcome::from_delivery(true, false)))
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::{native_combo, select_native_combo};

#[cfg(test)]
#[path = "select_combo_tests.rs"]
mod tests;
