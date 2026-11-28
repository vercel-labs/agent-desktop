//! Retry of a drill-down observation that the live application changed while
//! it was being read.
//!
//! A drill-down replaces refs, so it must observe its whole subtree. A live
//! container (a process list, a log, a feed) can drop or reorder rows while the
//! walk runs; that read is incomplete through no fault of the deadline. The
//! drill reads again with fresh handles until a read finishes whole or the
//! caller's `--timeout-ms` is spent.

use std::time::Duration;

use crate::{AdapterError, AppError, Deadline, ErrorCode};

const RETRY_PAUSE: Duration = Duration::from_millis(10);

/// One read of the drill subtree: whole, or cut short.
pub(super) enum DrillAttempt<T> {
    Complete(T),
    Incomplete(IncompleteRead),
}

/// Why a drill-down read ended short of the whole subtree.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum IncompleteCause {
    Deadline,
    LiveChurn,
    Bounded,
}

/// How a read ended short of the whole subtree.
#[derive(Clone, Copy)]
pub(super) struct IncompleteRead {
    pub(super) nodes_observed: usize,
    pub(super) cause: IncompleteCause,
}

/// Repeats `read` until it returns a whole subtree or the deadline is spent.
/// A deterministic observation bound (node/child cap or cells-in-view window)
/// is returned on the first read without spinning until the timeout expires.
pub(super) fn drill_until_complete<T>(
    deadline: Deadline,
    mut read: impl FnMut() -> Result<DrillAttempt<T>, AppError>,
) -> Result<T, AppError> {
    let mut attempts = 0_usize;
    let mut walks_finished = 0_usize;
    loop {
        attempts += 1;
        let incomplete = match read()? {
            DrillAttempt::Complete(value) => return Ok(value),
            DrillAttempt::Incomplete(incomplete) => incomplete,
        };
        if incomplete.cause == IncompleteCause::Bounded {
            return Err(drill_incomplete(incomplete, attempts, walks_finished));
        }
        walks_finished += usize::from(incomplete.cause == IncompleteCause::LiveChurn);
        if deadline.is_expired() {
            return Err(drill_incomplete(incomplete, attempts, walks_finished));
        }
        crate::wait::deadline::sleep(deadline.remaining().min(RETRY_PAUSE));
    }
}

fn drill_incomplete(
    incomplete: IncompleteRead,
    attempts: usize,
    walks_finished: usize,
) -> AppError {
    let (message, suggestion) = if incomplete.cause == IncompleteCause::Bounded {
        (
            "The drilled subtree exceeded the observation bound before it could be read whole, so no refs were replaced",
            "Drill a narrower subtree with --root, or use find to locate specific elements inside it",
        )
    } else if walks_finished == 0 {
        (
            "The drill-down read ran out of time before it saw the whole subtree, so no refs were replaced",
            "Retry with a larger --timeout-ms, or drill a narrower subtree",
        )
    } else {
        (
            "The drilled subtree kept changing while it was read, so no refs were replaced",
            "Drill a narrower subtree, or retry when the application stops updating it",
        )
    };
    AppError::Adapter(
        AdapterError::new(ErrorCode::Timeout, message)
            .with_suggestion(suggestion)
            .with_details(serde_json::json!({
                "kind": "observation_incomplete",
                "nodes_observed": incomplete.nodes_observed,
                "attempts": attempts,
                "walks_finished": walks_finished,
            })),
    )
}

#[cfg(test)]
#[path = "drill_retry_tests.rs"]
mod tests;
