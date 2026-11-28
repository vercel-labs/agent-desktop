//! Scrolling a native list box item into view through `LB_SETTOPINDEX`.
//!
//! ScrollItemPattern fails on a WinForms ListBox item, and the ancestor
//! ScrollPattern ladder could not bring it into view (W-W0-16).
//! `LB_SETTOPINDEX` on the list box window brings the item into view with 0
//! foreground events in two sessions (C9). A list box is a window that
//! answers `GetListBoxInfo`: the user32 control or a superclass of it.

use agent_desktop_core::AdapterError;

use crate::actions::select_combo::index_or_fall_through;

/// Puts the item at the top once a name search found it. `Ok(false)` when the
/// search found no item or the list box refused the index, so the caller
/// tries its next route; any other search failure stays an error.
pub(crate) fn top_item_or_fall_through(
    found: Result<usize, AdapterError>,
    set_top: impl FnOnce(usize) -> Result<bool, AdapterError>,
) -> Result<bool, AdapterError> {
    match index_or_fall_through(found)? {
        Some(index) => set_top(index),
        None => Ok(false),
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::top_item_or_fall_through;
    use crate::actions::native_message::{find_unique, send};
    use crate::actions::native_window::NativeWindow;
    use crate::tree::automation::automation_client;
    use crate::tree::element::UIAElement;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use agent_desktop_core::{AdapterError, Deadline, LocatorField};
    use windows_sys::Win32::UI::Controls::GetListBoxInfo;
    use windows_sys::Win32::UI::WindowsAndMessaging::{LB_ERR, LB_FINDSTRINGEXACT, LB_SETTOPINDEX};

    /// The list box window that owns the item, and the item's name. The list
    /// box must belong to the item's verified process.
    pub(crate) fn native_list_box_item(element: &UIAElement) -> Option<(NativeWindow, String)> {
        let LocatorField::Known(name) = read_one(element, TreeProperty::Name).text() else {
            return None;
        };
        let walker = automation_client().ok()?.get_raw_view_walker().ok()?;
        let parent = walker.get_parent(&element.0).ok()?;
        let hwnd: isize = parent.get_native_window_handle().ok()?.into();
        let list = NativeWindow::of(hwnd, element)?;
        (unsafe { GetListBoxInfo(hwnd as *mut std::ffi::c_void) } > 0).then_some((list, name))
    }

    /// Puts the named item at the top of the list box. `Ok(false)` when the
    /// list box has no such item or refused the index.
    pub(crate) fn scroll_list_box_item(
        list: &NativeWindow,
        name: &str,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        top_item_or_fall_through(
            find_unique(list, LB_FINDSTRINGEXACT, name, deadline),
            |index| Ok(send(list, LB_SETTOPINDEX, index, 0, deadline)? != LB_ERR as isize),
        )
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::{native_list_box_item, scroll_list_box_item};

#[cfg(test)]
#[path = "scroll_list_box_tests.rs"]
mod tests;
