use std::time::Duration;

use agent_desktop_core::{AdapterError, ErrorCode};

use crate::system::window::native_group::is_unproven;

/// How long an attach keeps asking an application that does not answer for
/// its windows. An attach sends nothing, so waiting longer cannot change what
/// the application will say; a spawned application is different, because its
/// windows are still being created.
pub(super) const ATTACH_READ_GRACE: Duration = Duration::from_millis(2_000);

/// A window read that says nothing about the windows: the application did
/// not answer, or the display could not prove them.
pub(super) fn is_window_unreadable(error: &AdapterError) -> bool {
    is_unproven(error)
        || matches!(
            error.code,
            ErrorCode::Timeout
                | ErrorCode::AppUnresponsive
                | ErrorCode::AppNotFound
                | ErrorCode::WindowNotFound
        )
}
