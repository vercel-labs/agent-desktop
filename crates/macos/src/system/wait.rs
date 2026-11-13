use agent_desktop_core::{AdapterError, Deadline, ProcessIdentity};

#[cfg(target_os = "macos")]
pub(crate) fn wait_for_menu(
    process: ProcessIdentity,
    open: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    use crate::system::process::process_identity::require_core;
    let read = || {
        let identity = require_core(&process)?;
        crate::tree::surfaces::is_menu_open(identity.pid(), deadline).map(|state| state == open)
    };
    let step = std::time::Duration::from_millis(50);
    if deadline.poll_until(deadline.remaining(), step, read, |read| {
        !matches!(read, Ok(false))
    })? {
        require_core(&process)?;
        return Ok(());
    }
    let message = if open {
        "No context menu opened before the deadline"
    } else {
        "Context menu did not close before the deadline"
    };
    Err(deadline.timeout_error().with_platform_detail(message))
}
