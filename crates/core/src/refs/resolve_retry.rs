//! The one retry loop for a resolution whose reads came back incomplete.

use std::time::Duration;

use crate::{AdapterError, Deadline, ErrorCode, refs::resolve_errors::mark_deadline_elapsed};

const FIRST_RETRY_PAUSE: Duration = Duration::from_millis(25);
const MAXIMUM_RETRY_PAUSE: Duration = Duration::from_millis(400);

/// Runs `operation` until it settles or `deadline` passes, retrying only a
/// transient incomplete read ([`AdapterError::is_transient_incomplete_read`]).
///
/// The pause doubles from 25 ms to 400 ms: a briefly busy provider is caught
/// by the opening polls, and one that keeps failing is not asked forty times a
/// second for the whole budget. When the budget runs out after an incomplete
/// read, the caller gets that diagnosis stamped `deadline_elapsed` instead of
/// a bare timeout.
pub fn retry_incomplete_until<T>(
    deadline: Deadline,
    mut operation: impl FnMut() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    let mut last_incomplete = None;
    let mut pause = FIRST_RETRY_PAUSE;
    loop {
        if deadline.is_expired() {
            return Err(last_incomplete
                .map(mark_deadline_elapsed)
                .unwrap_or_else(|| deadline.timeout_error()));
        }
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if error.is_transient_incomplete_read() => {
                last_incomplete = Some(error);
                deadline.pause(pause);
                pause = (pause * 2).min(MAXIMUM_RETRY_PAUSE);
            }
            Err(error) if error.code == ErrorCode::Timeout => {
                return Err(last_incomplete.map_or(error, mark_deadline_elapsed));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
#[path = "resolve_retry_tests.rs"]
mod tests;
