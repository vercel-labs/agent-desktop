use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, NotificationFilter, NotificationInfo,
};

use super::list::list_infos;

const REOBSERVATION_INTERVAL: Duration = Duration::from_millis(25);
const REMOVAL_SETTLE_TIME: Duration = Duration::from_millis(2_000);

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;

/// The identity a mutation verified against: source application, title and
/// body. Mutable control values never participate - an entry whose toggle
/// state or progress changed is still the entry that was targeted.
pub(super) fn same_identity(original: &NotificationInfo, current: &NotificationInfo) -> bool {
    original.app_name == current.app_name
        && original.title == current.title
        && original.body == current.body
}

pub(super) fn matching_count(entries: &[NotificationInfo], target: &NotificationInfo) -> usize {
    entries
        .iter()
        .filter(|current| same_identity(target, current))
        .count()
}

pub(super) fn entry_gone(target: &NotificationInfo, current: &[NotificationInfo]) -> bool {
    matching_count(current, target) == 0
}

/// The failure one captured member still present after the clear reports
/// when no invoke error was recorded for it: the clear reached the shell, so
/// the entry's survival is delivered-but-unverified, never not-delivered.
pub(super) fn survivor_failure() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "notification from the captured set is still present",
    )
    .with_disposition(DeliverySemantics::delivered_unverified())
}

/// Whether the entry an action targeted is gone or no longer carries the
/// identity it was read with - [`entry_gone`] under the name this call site's
/// question reads by.
pub(super) fn action_changed_state(
    original: &NotificationInfo,
    current: &[NotificationInfo],
) -> bool {
    entry_gone(original, current)
}

pub(super) fn dismiss_survived_error(index: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "The dismiss of the notification at index {index} was invoked but the entry is still present"
        ),
    )
    .with_suggestion(
        "Retry the dismissal: the shell can accept an invoke without acting on it"
    )
    .with_disposition(DeliverySemantics::delivered_unverified())
}

/// Re-reads the center until every target has vanished or the settle window
/// closes, and returns the last listing either way.
///
/// The shell accepts an invoke and applies it a moment later, so the read that
/// judges a mutation is a polled read, not the first one. An expired settle is
/// an answer (the targets survived the window), never an error - the caller
/// decides what survival means.
pub(super) fn read_settling_without(
    targets: &[NotificationInfo],
    hwnd: isize,
    filter: &NotificationFilter,
    deadline: Deadline,
) -> Result<Vec<NotificationInfo>, AdapterError> {
    let settle = deadline.capped(REMOVAL_SETTLE_TIME);
    let mut current = list_infos(filter, hwnd, settle)?;
    while !targets.iter().all(|target| entry_gone(target, &current)) {
        if settle.is_expired() {
            return Ok(current);
        }
        std::thread::sleep(settle.remaining_slice(REOBSERVATION_INTERVAL)?);
        current = match list_infos(filter, hwnd, settle) {
            Ok(listed) => listed,
            Err(error) if error.code == ErrorCode::Timeout => return Ok(current),
            Err(error) => return Err(error),
        };
    }
    Ok(current)
}
