//! A native control window to message and the process instance it belongs to.

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, ProcessId};

use crate::tree::element::UIAElement;

/// A user32 control window, with the process instance that owned it when
/// the target element was verified. Every message first checks that the
/// window still belongs to that instance, so a window handle the system
/// recycled for another process is not messaged. The check and the message
/// are two calls, not one atomic step; the check narrows the window in which
/// a recycled handle could be reached to the gap between them.
pub(crate) struct NativeWindow {
    pub(crate) hwnd: isize,
    pid: u32,
    instance: String,
}

impl NativeWindow {
    /// `hwnd` as the window of `element`'s verified process. `None` when the
    /// element has no verified process identity, so the message route is
    /// not offered.
    pub(crate) fn of(hwnd: isize, element: &UIAElement) -> Option<Self> {
        let (pid, instance) = element.verified_process()?;
        (hwnd != 0).then(|| Self {
            hwnd,
            pid,
            instance: instance.to_owned(),
        })
    }

    /// Fails closed unless `hwnd`, this window or one it notifies, is still
    /// owned by the recorded process instance.
    pub(crate) fn ensure_owned(&self, hwnd: isize) -> Result<(), AdapterError> {
        let pid = ProcessId::new(self.pid);
        let owner = crate::system::window::window_identity::live_window_owner(
            hwnd as crate::system::window::window_enum::WindowHandle,
        );
        if owner == Some(pid)
            && crate::system::process_identity::matches_instance(pid, &self.instance)?
        {
            return Ok(());
        }
        Err(AdapterError::new(
            ErrorCode::StaleRef,
            "The native control window no longer belongs to the target process",
        )
        .with_details(serde_json::json!({ "kind": "native_window_owner_changed" }))
        .with_disposition(DeliverySemantics::not_delivered()))
    }
}

#[cfg(test)]
#[path = "native_window_tests.rs"]
mod tests;
