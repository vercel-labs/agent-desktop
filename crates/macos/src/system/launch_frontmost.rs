use agent_desktop_core::Deadline;

use super::app::workspace_apps::{WindowOwner, focused_owner_until};
use super::process::process_identity;

/// The attach-and-activate postcondition: the frontmost application must be
/// the exact process instance the launch attached to. A read that fails or
/// names another owner leaves the delivery unverified rather than failing a
/// launch that already succeeded.
pub(crate) fn frontmost_is(pid: i32, process_instance: &str, deadline: Deadline) -> bool {
    let Ok(Some(owner)) = focused_owner_until(deadline) else {
        return false;
    };
    owner_is_target(&owner, pid, process_instance)
}

pub(crate) fn owner_is_target(owner: &WindowOwner, pid: i32, process_instance: &str) -> bool {
    owner.pid == pid
        && owner.launch_time.is_some_and(|launch_time| {
            process_identity::instance_matches_launch_time(owner.pid, process_instance, launch_time)
                .unwrap_or(false)
        })
}
