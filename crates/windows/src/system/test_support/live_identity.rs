//! Live process identity builder shared by live test modules across the crate.

use agent_desktop_core::{ProcessId, ProcessIdentity};

/// The identity a live pid actually carries right now: its process-instance
/// token, read fresh rather than assumed, so a test asserts against what the
/// OS reports instead of a value it fabricated.
pub(crate) fn live_process_identity(pid: ProcessId) -> ProcessIdentity {
    let token = crate::system::process_identity::token_for_pid(pid)
        .expect("token read")
        .expect("live token");
    ProcessIdentity::new(pid, token)
}
