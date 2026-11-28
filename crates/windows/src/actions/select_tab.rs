//! Selection of a tab in a native tab control through `TCM_SETCURFOCUS`.
//!
//! `TCM_SETCURSEL` and the accessibility select route set the control's
//! current index without sending `TCN_SELCHANGE` to the parent, so the
//! selected state reads true while the page never changes. `TCM_SETCURFOCUS`
//! changes the selection and sends `TCN_SELCHANGING` and `TCN_SELCHANGE`, and
//! the page changes with no foreground event on a window in the background
//! (A31-6). A tab control is a window of class `SysTabControl32` or a
//! superclass of it (WinForms). The item's index is its position among the
//! tab items of that control. A control whose item count disagrees with the
//! tree, or whose selection did not follow the focus, hands the selection
//! back to the caller's own route.

use agent_desktop_core::DeliveryOutcome;

pub(crate) const TAB_SELECT_LABEL: &str = "Win32.TCM_SETCURFOCUS";

/// Whether a window class is the user32 tab control or a superclass of it.
pub(crate) fn is_native_tab_class(class: &str) -> bool {
    class.contains("SysTabControl32")
}

/// The index to select: the item's position when the control reports as many
/// items as the tree shows, `None` when the two disagree.
pub(crate) fn tab_index(position: usize, siblings: usize, item_count: isize) -> Option<usize> {
    (usize::try_from(item_count).ok()? == siblings && position < siblings).then_some(position)
}

/// Verified delivery when the control's current selection is `index`. `None`
/// when the focus moved without the selection, as under `TCS_BUTTONS`, so the
/// caller's own route runs.
pub(crate) fn selection_outcome(current: isize, index: usize) -> Option<DeliveryOutcome> {
    (usize::try_from(current).ok() == Some(index))
        .then(|| DeliveryOutcome::from_delivery(true, true))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{is_native_tab_class, selection_outcome, tab_index};
    use crate::actions::native_message::send;
    use crate::actions::native_window::NativeWindow;
    use crate::tree::automation::automation_client;
    use crate::tree::element::UIAElement;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use agent_desktop_core::{AdapterError, Deadline, DeliveryOutcome};
    use uiautomation::types::ControlType;
    use windows_sys::Win32::UI::Controls::{TCM_GETCURSEL, TCM_GETITEMCOUNT, TCM_SETCURFOCUS};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;

    pub(crate) fn is_tab_item(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::ControlType).number() == Some(ControlType::TabItem as i32)
    }

    fn window_class(hwnd: isize) -> String {
        let mut buffer = [0u16; 256];
        let length =
            unsafe { GetClassNameW(hwnd as *mut std::ffi::c_void, buffer.as_mut_ptr(), 256) };
        String::from_utf16_lossy(&buffer[..usize::try_from(length).unwrap_or(0)])
    }

    fn position_among_tabs(element: &UIAElement) -> Option<(NativeWindow, usize, usize)> {
        let client = automation_client().ok()?;
        let walker = client.get_raw_view_walker().ok()?;
        let parent = walker.get_parent(&element.0).ok()?;
        let hwnd: isize = parent.get_native_window_handle().ok()?.into();
        let tab = NativeWindow::of(hwnd, element)?;
        let mut position = None;
        let mut siblings = 0;
        let mut child = walker.get_first_child(&parent).ok();
        while let Some(current) = child {
            if current.get_control_type().ok() == Some(ControlType::TabItem) {
                if client
                    .compare_elements(&current, &element.0)
                    .unwrap_or(false)
                {
                    position = Some(siblings);
                }
                siblings += 1;
            }
            child = walker.get_next_sibling(&current).ok();
        }
        Some((tab, position?, siblings))
    }

    /// Selects the tab item through its native tab control. `Ok(None)` when
    /// the item is not in a native tab control, so the caller's own route
    /// runs.
    pub(crate) fn select_native_tab(
        element: &UIAElement,
        deadline: Deadline,
    ) -> Result<Option<DeliveryOutcome>, AdapterError> {
        if !is_tab_item(element) {
            return Ok(None);
        }
        let Some((tab, position, siblings)) = position_among_tabs(element) else {
            return Ok(None);
        };
        if !is_native_tab_class(&window_class(tab.hwnd)) {
            return Ok(None);
        }
        let count = send(&tab, TCM_GETITEMCOUNT, 0, 0, deadline)?;
        let Some(index) = tab_index(position, siblings, count) else {
            return Ok(None);
        };
        send(&tab, TCM_SETCURFOCUS, index, 0, deadline)?;
        let current = send(&tab, TCM_GETCURSEL, 0, 0, deadline)?;
        Ok(selection_outcome(current, index))
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::{is_tab_item, select_native_tab};

#[cfg(test)]
#[path = "select_tab_tests.rs"]
mod tests;
