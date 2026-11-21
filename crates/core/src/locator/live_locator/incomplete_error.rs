//! The one place a locator that cannot prove its result becomes an error.
//!
//! The cause comes from the stats of the observation, so the message and the
//! suggestion name the lever that helps: a larger `--timeout-ms` only when the
//! deadline ran out, a narrower query when more time cannot change the answer.

use super::{LocatorResolution, LocatorStats, resolve::has_deterministic_limit};
use crate::{AdapterError, ErrorCode};
use serde_json::json;

const DEADLINE_SUGGESTION: &str =
    "Retry with a larger --timeout-ms, or narrow the query with --role or --name.";

const NARROW_THE_QUERY_SUGGESTION: &str = "More time will not help. Narrow the query with --role or --name, or run snapshot, which returns the readable part of the tree with complete:false.";

#[derive(Clone, Copy, PartialEq, Eq)]
enum IncompleteCause {
    Deadline,
    TraversalLimit,
    UnreadableElements,
}

impl IncompleteCause {
    fn of(stats: &LocatorStats) -> Self {
        let health = &stats.reads.health;
        if health.deadline_exhausted > 0 {
            Self::Deadline
        } else if health.native_read_failures == 0 && has_deterministic_limit(stats) {
            Self::TraversalLimit
        } else {
            Self::UnreadableElements
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Deadline => "deadline",
            Self::TraversalLimit => "traversal_limit",
            Self::UnreadableElements => "unreadable_elements",
        }
    }

    fn suggestion(self) -> &'static str {
        match self {
            Self::Deadline => DEADLINE_SUGGESTION,
            Self::TraversalLimit | Self::UnreadableElements => NARROW_THE_QUERY_SUGGESTION,
        }
    }

    fn message(self, stats: &LocatorStats) -> String {
        match self {
            Self::Deadline => {
                "The locator search ran out of time before it could prove its result".to_owned()
            }
            Self::TraversalLimit => format!(
                "The locator search hit a traversal limit ({}) and cannot prove its result",
                limits_hit(stats).join(", ")
            ),
            Self::UnreadableElements => match failed_reads(stats) {
                0 => "Part of the tree could not be read, so the locator search cannot prove its result".to_owned(),
                1 => "1 accessibility read failed, so the locator search cannot prove its result".to_owned(),
                count => format!(
                    "{count} accessibility reads failed, so the locator search cannot prove its result"
                ),
            },
        }
    }
}

fn failed_reads(stats: &LocatorStats) -> u64 {
    stats.reads.health.native_read_failures + stats.reads.health.cannot_complete
}

fn limits_hit(stats: &LocatorStats) -> Vec<&'static str> {
    let limits = &stats.traversal.limits;
    [
        ("node", limits.node_hits),
        ("edge", limits.edge_hits),
        ("child", limits.child_hits),
        ("text", limits.text_hits),
        ("depth", limits.depth_hits),
    ]
    .into_iter()
    .filter(|(_, hits)| *hits > 0)
    .map(|(name, _)| name)
    .collect()
}

/// Builds the `TIMEOUT` for a resolution that could not prove its result.
/// `details.kind` is `locator_incomplete`; `details.cause` is `deadline`,
/// `unreadable_elements` or `traversal_limit`, and `details.retryable` is true
/// only for the deadline.
pub fn incomplete_locator_error(resolution: &LocatorResolution) -> AdapterError {
    let cause = IncompleteCause::of(&resolution.stats);
    AdapterError::new(ErrorCode::Timeout, cause.message(&resolution.stats))
        .with_suggestion(cause.suggestion())
        .with_details(json!({
            "kind": "locator_incomplete",
            "cause": cause.as_str(),
            "retryable": cause == IncompleteCause::Deadline,
            "observed_matches": resolution.meta.total_matches,
            "roles_present": resolution.meta.roles_present,
            "query_stats": resolution.stats,
        }))
        .with_disposition(crate::DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "incomplete_error_tests.rs"]
mod tests;
