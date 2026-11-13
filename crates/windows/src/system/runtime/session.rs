//! Session-scoped COM MTA usage for persistent Windows hosts.

use agent_desktop_core::{AdapterError, AdapterSession, Deadline, ErrorCode};

use crate::system::runtime::hresult::com_hresult_detail;
use crate::system::runtime::mta_usage::MtaUsage;

/// Owns a session-scoped COM MTA usage registration, separate from the
/// process-lifetime one. Because COM permits releasing an MTA cookie from any
/// thread, `close` and `Drop` both release it and whichever runs first wins.
pub(crate) struct WindowsAdapterSession {
    mta_usage: MtaUsage,
}

pub(crate) fn open(deadline: Deadline) -> Result<WindowsAdapterSession, AdapterError> {
    open_with(deadline, MtaUsage::acquire)
}

fn open_with(
    deadline: Deadline,
    acquire: impl FnOnce() -> Result<MtaUsage, i32>,
) -> Result<WindowsAdapterSession, AdapterError> {
    deadline.ensure_remaining()?;
    let mta_usage = acquire().map_err(mta_usage_acquire_failure)?;
    Ok(WindowsAdapterSession { mta_usage })
}

impl AdapterSession for WindowsAdapterSession {
    fn close(mut self: Box<Self>) -> Result<(), AdapterError> {
        self.mta_usage.release().map_err(mta_usage_release_failure)
    }
}

impl Drop for WindowsAdapterSession {
    fn drop(&mut self) {
        let _ = self.mta_usage.release();
    }
}

fn mta_usage_acquire_failure(hresult: i32) -> AdapterError {
    AdapterError::new(
        ErrorCode::Internal,
        "Session-scoped COM MTA usage could not be registered",
    )
    .with_platform_detail(com_hresult_detail(hresult))
    .with_suggestion("Verify the host process allows COM initialization, then reopen the session")
}

fn mta_usage_release_failure(hresult: i32) -> AdapterError {
    AdapterError::new(
        ErrorCode::Internal,
        "Session-scoped COM MTA usage could not be released",
    )
    .with_platform_detail(com_hresult_detail(hresult))
    .with_suggestion(
        "Treat the session as closed; the process retains one leaked MTA usage until exit",
    )
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
