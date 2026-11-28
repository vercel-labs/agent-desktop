use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, NotificationFilter, NotificationInfo,
};

use super::actions::{build_filter, invoke_dismiss, invoke_element};
use super::list::list_entries;
use super::verify::{entry_gone, read_settling_without, same_identity, survivor_failure};

#[cfg(test)]
#[path = "dismiss_all_tests.rs"]
mod tests;

pub(super) fn dismiss_all_impl(
    app_filter: Option<&str>,
    hwnd: isize,
    deadline: Deadline,
) -> Result<(Vec<NotificationInfo>, Vec<AdapterError>), AdapterError> {
    let filter = build_filter(app_filter);
    let entries = list_entries(&filter, hwnd, deadline).map_err(AdapterError::unsent)?;
    if entries.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let captured: Vec<NotificationInfo> = entries.into_iter().map(|entry| entry.info).collect();
    let mut invoke_errors: Vec<(usize, AdapterError)> = Vec::new();
    if app_filter.is_some() {
        dismiss_each_by_freshly_read_identity(
            &captured,
            &filter,
            hwnd,
            deadline,
            &mut invoke_errors,
        )?;
    } else {
        let root = crate::tree::automation::root_from_hwnd(hwnd, deadline)
            .map_err(AdapterError::unsent)?;
        let clear = super::read::find_clear_all_button(&root)
            .map_err(AdapterError::unsent)?
            .ok_or_else(super::read::missing_clear_all_error)?;
        invoke_element(&clear)?;
    }
    let current = read_settling_without(&captured, hwnd, &filter, deadline)?;
    let failures = dismiss_all_failures(&captured, &current, &invoke_errors);
    let dismissed = captured
        .iter()
        .filter(|member| entry_gone(member, &current))
        .cloned()
        .collect();
    Ok((dismissed, failures))
}

/// Dismisses each captured target through a freshly re-read live element,
/// never through the handle or index the outer capture took.
///
/// A filtered dismissal only ever narrows which entries are targeted; it must
/// never fall back to the Action-Center-wide clear-all, because that control
/// has no notion of "this app's entries" and would destroy every other
/// application's notifications along with the ones the caller asked to
/// remove. Removing one entry can renumber and recycle the elements after it
/// in the center's virtualized list, so every iteration re-reads the live
/// surface with the same filter and matches its target by identity - never by
/// the index or element handle the outer capture took, which may already be
/// stale by the time this entry's turn comes. An invoke's own error is
/// recorded against the target's captured index rather than discarded, so the
/// caller's survivor comparison can report the real reason a filtered entry
/// refused to leave, not just that it remained.
fn dismiss_each_by_freshly_read_identity(
    captured: &[NotificationInfo],
    filter: &NotificationFilter,
    hwnd: isize,
    deadline: Deadline,
    invoke_errors: &mut Vec<(usize, AdapterError)>,
) -> Result<(), AdapterError> {
    dismiss_each_with(
        captured,
        || {
            list_entries(filter, hwnd, deadline).map(|entries| {
                entries
                    .into_iter()
                    .map(|entry| (entry.info, entry.element))
                    .collect()
            })
        },
        invoke_dismiss,
        invoke_errors,
    )
}

/// Runs one dismissal per captured target through a surface read again
/// immediately before each invoke. An invoke error is recorded against the
/// target's captured index instead of abandoning the batch. A listing that
/// fails mid-batch is stamped not-delivered only while nothing has been
/// invoked yet, because a later read cannot un-start a dispatch an earlier
/// target already sent.
fn dismiss_each_with<E, R, I>(
    captured: &[NotificationInfo],
    read_live: R,
    invoke: I,
    invoke_errors: &mut Vec<(usize, AdapterError)>,
) -> Result<(), AdapterError>
where
    R: Fn() -> Result<Vec<(NotificationInfo, E)>, AdapterError>,
    I: Fn(&E, usize) -> Result<(), AdapterError>,
{
    let mut dispatched = false;
    for target in captured {
        let live = read_live().map_err(|error| batch_read_failure(error, dispatched))?;
        let Some(current) = live.iter().find(|(info, _)| same_identity(target, info)) else {
            continue;
        };
        match invoke(&current.1, current.0.index) {
            Ok(()) => dispatched = true,
            Err(error) => {
                if error.disposition != DeliverySemantics::not_delivered() {
                    dispatched = true;
                }
                invoke_errors.push((target.index, error));
            }
        }
    }
    Ok(())
}

fn batch_read_failure(error: AdapterError, dispatched: bool) -> AdapterError {
    if dispatched {
        error.with_disposition(DeliverySemantics::delivered_unverified())
    } else {
        error.unsent()
    }
}

/// One failure line per captured member still present after the clear,
/// carrying the specific invoke error a filtered dismissal recorded for it
/// when one exists, the generic survivor message otherwise - never both, and
/// never for a member the settle read proved gone. A member that vanished
/// during the clear is reported as dismissed even if its own invoke errored,
/// because the settle read is the proof the caller's envelope trusts, not the
/// invoke's own return value.
fn dismiss_all_failures(
    captured: &[NotificationInfo],
    current: &[NotificationInfo],
    invoke_errors: &[(usize, AdapterError)],
) -> Vec<AdapterError> {
    captured
        .iter()
        .filter(|member| !entry_gone(member, current))
        .map(|member| {
            let mut failure = invoke_errors
                .iter()
                .find(|(index, _)| *index == member.index)
                .map(|(_, error)| error.clone())
                .unwrap_or_else(survivor_failure);
            failure.message = format!("#{}: {}", member.index, failure.message);
            failure
        })
        .collect()
}
