//! Window messages to native user32 controls in another process.
//!
//! Combo boxes and list boxes answer their own `CB_` and `LB_` messages, and
//! user32 marshals their string arguments across processes. Each send is
//! bounded by the action deadline and returns at once when the target thread
//! is hung (`SMTO_ABORTIFHUNG`).

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

/// The unique index a name names, from two exact searches: one from the top
/// and one starting after the first hit, which wraps back to it when the
/// name occurs once.
pub(crate) fn unique_index(first: isize, again: isize) -> Result<usize, AdapterError> {
    if first < 0 {
        return Err(AdapterError::new(
            ErrorCode::ElementNotFound,
            "No native control item matched the requested name",
        )
        .with_disposition(DeliverySemantics::not_delivered())
        .with_suggestion("Use find or snapshot to inspect the available items."));
    }
    if again >= 0 && again != first {
        return Err(AdapterError::ambiguous_target(
            "Multiple native control items share the requested name",
        )
        .with_details(serde_json::json!({ "kind": "ambiguous_select_value" })));
    }
    usize::try_from(first).map_err(|_| AdapterError::internal("native control index overflow"))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{AdapterError, DeliverySemantics, ErrorCode, unique_index};
    use agent_desktop_core::Deadline;
    use std::ffi::c_void;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SendMessageTimeoutW};

    const SEND_CAP_MS: u64 = 2_000;

    pub(crate) fn send(
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        deadline: Deadline,
    ) -> Result<isize, AdapterError> {
        let budget = deadline
            .remaining()
            .as_millis()
            .min(u128::from(SEND_CAP_MS)) as u32;
        let mut answer: usize = 0;
        let sent = unsafe {
            SendMessageTimeoutW(
                hwnd as *mut c_void,
                message,
                wparam,
                lparam,
                SMTO_ABORTIFHUNG,
                budget.max(1),
                &mut answer,
            )
        };
        if sent == 0 {
            return Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "The native control did not answer its window message in time",
            )
            .with_disposition(DeliverySemantics::uncertain()));
        }
        Ok(answer as isize)
    }

    /// The index of the item named `name`, found with an exact-match message
    /// such as `CB_FINDSTRINGEXACT` or `LB_FINDSTRINGEXACT`.
    pub(crate) fn find_unique(
        hwnd: isize,
        find_message: u32,
        name: &str,
        deadline: Deadline,
    ) -> Result<usize, AdapterError> {
        if name.contains('\0') {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "A native control item name cannot contain a NUL character",
            )
            .with_disposition(DeliverySemantics::not_delivered()));
        }
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let find = |start: isize| {
            send(
                hwnd,
                find_message,
                start as usize,
                wide.as_ptr() as isize,
                deadline,
            )
        };
        let first = find(-1)?;
        unique_index(first, if first >= 0 { find(first)? } else { -1 })
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::{find_unique, send};

#[cfg(test)]
#[path = "native_message_tests.rs"]
mod tests;
