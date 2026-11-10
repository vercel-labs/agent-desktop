//! Dismisses every captured notification against a fresh listing per entry.

use agent_desktop_core::{AdapterError, DeliverySemantics, NotificationInfo};

/// Dismisses the captured entries newest first, each against a fresh listing.
/// A listing failure ends the loop. After a dispatch it is recorded as a
/// failure beside the dismissals already made, so the caller still merges a
/// true disposition. An entry already gone counts as dismissed without a
/// send, so a listing failure after only such entries stays `not_delivered`;
/// with nothing dismissed at all it is the command's error.
#[cfg(target_os = "macos")]
pub(super) fn dismiss_captured(
    entries: &[super::list::NotificationEntry],
    mut list: impl FnMut() -> Result<Vec<super::list::NotificationEntry>, AdapterError>,
    mut attempt: impl FnMut(&super::list::NotificationEntry, usize) -> Result<(), AdapterError>,
) -> Result<(Vec<NotificationInfo>, Vec<AdapterError>), AdapterError> {
    let mut dismissed = Vec::new();
    let mut failures: Vec<AdapterError> = Vec::new();
    let mut sent = false;

    for original in entries.iter().rev() {
        let current_entries = match list() {
            Ok(current_entries) => current_entries,
            Err(mut error) => {
                let sent = sent
                    || failures
                        .iter()
                        .any(|failure| failure.disposition != DeliverySemantics::not_delivered());
                if !sent && dismissed.is_empty() {
                    return Err(error);
                }
                if !sent {
                    error.disposition = DeliverySemantics::not_delivered();
                }
                error.message = format!(
                    "Stopped after {} of {} notifications: {}",
                    dismissed.len() + failures.len(),
                    entries.len(),
                    error.message
                );
                failures.push(error);
                break;
            }
        };
        match dismiss_turn(original, &current_entries, |current, matching_before| {
            let outcome = attempt(current, matching_before);
            sent |= outcome.is_ok();
            outcome
        }) {
            Ok(()) => dismissed.push(original.info.clone()),
            Err(message) => failures.push(message),
        }
    }

    Ok((dismissed, failures))
}

/// Runs one captured entry's turn against a fresh listing. An entry the
/// listing no longer holds already satisfies the dismissal, so it counts as
/// dismissed without an attempt; a held entry is attempted, and its failure
/// is reported against the captured index.
#[cfg(target_os = "macos")]
fn dismiss_turn(
    original: &super::list::NotificationEntry,
    current_entries: &[super::list::NotificationEntry],
    attempt: impl FnOnce(&super::list::NotificationEntry, usize) -> Result<(), AdapterError>,
) -> Result<(), AdapterError> {
    let Some(current) = current_entries
        .iter()
        .find(|entry| super::dismiss_verify::matches(original, entry))
    else {
        return Ok(());
    };
    let matching_before = super::dismiss_verify::matching_count(current_entries, current);
    attempt(current, matching_before).map_err(|mut error| {
        error.message = format!("#{}: {}", original.info.index, error.message);
        error
    })
}

#[cfg(test)]
#[path = "dismiss_all_tests.rs"]
mod tests;
