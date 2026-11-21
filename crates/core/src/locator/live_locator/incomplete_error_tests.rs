use super::incomplete_locator_error;
use crate::locator::live_locator::{LocatorResolution, LocatorResolutionMeta, LocatorStats};
use crate::{DeliverySemantics, ErrorCode};

fn resolution_with(stats: LocatorStats) -> LocatorResolution {
    LocatorResolution {
        matches: Vec::new(),
        refmap: None,
        stats,
        meta: LocatorResolutionMeta {
            total_matches: 3,
            complete: false,
            selection_complete: false,
            truncated: false,
            roles_present: vec!["button".to_owned()],
        },
    }
}

fn error_parts(stats: LocatorStats) -> (String, String, serde_json::Value) {
    let error = incomplete_locator_error(&resolution_with(stats));
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    (
        error.message,
        error
            .suggestion
            .expect("an incomplete locator suggests a remedy"),
        error
            .details
            .expect("an incomplete locator carries details"),
    )
}

#[test]
fn an_exhausted_deadline_suggests_a_larger_timeout_and_is_retryable() {
    let mut stats = LocatorStats::default();
    stats.reads.health.deadline_exhausted = 1;

    let (message, suggestion, details) = error_parts(stats);

    assert!(message.contains("ran out of time"), "{message}");
    assert!(suggestion.contains("larger --timeout-ms"), "{suggestion}");
    assert_eq!(details["kind"], "locator_incomplete");
    assert_eq!(details["cause"], "deadline");
    assert_eq!(details["retryable"], true);
    assert_eq!(details["observed_matches"], 3);
    assert_eq!(details["roles_present"][0], "button");
    assert_eq!(details["query_stats"]["reads"]["deadline_exhausted"], 1);
}

#[test]
fn unreadable_elements_say_more_time_will_not_help() {
    let mut stats = LocatorStats::default();
    stats.reads.health.native_read_failures = 2;
    stats.traversal.nodes_visited = 808;

    let (message, suggestion, details) = error_parts(stats);

    assert!(
        message.contains("2 accessibility reads failed"),
        "{message}"
    );
    assert!(
        suggestion.contains("More time will not help"),
        "{suggestion}"
    );
    assert!(suggestion.contains("--role") && suggestion.contains("--name"));
    assert!(suggestion.contains("snapshot") && suggestion.contains("complete:false"));
    assert!(!suggestion.contains("larger --timeout-ms"), "{suggestion}");
    assert_eq!(details["kind"], "locator_incomplete");
    assert_eq!(details["cause"], "unreadable_elements");
    assert_eq!(details["retryable"], false);
    assert_eq!(details["query_stats"]["reads"]["native_read_failures"], 2);
}

#[test]
fn a_traversal_limit_names_the_limit_and_is_not_retryable() {
    let mut stats = LocatorStats::default();
    stats.traversal.limits.node_hits = 1;
    stats.traversal.limits.depth_hits = 4;

    let (message, suggestion, details) = error_parts(stats);

    assert!(message.contains("(node, depth)"), "{message}");
    assert!(
        suggestion.contains("More time will not help"),
        "{suggestion}"
    );
    assert_eq!(details["cause"], "traversal_limit");
    assert_eq!(details["retryable"], false);
}

#[test]
fn the_deadline_outranks_failed_reads_and_failed_reads_outrank_a_limit() {
    let mut stats = LocatorStats::default();
    stats.traversal.limits.node_hits = 1;
    stats.reads.health.native_read_failures = 1;
    let (_, _, details) = error_parts(stats.clone());
    assert_eq!(details["cause"], "unreadable_elements");

    stats.reads.health.deadline_exhausted = 1;
    let (_, _, details) = error_parts(stats);
    assert_eq!(details["cause"], "deadline");
}

#[test]
fn an_incomplete_result_with_no_counter_is_reported_as_unreadable_without_a_count() {
    let (message, _, details) = error_parts(LocatorStats::default());

    assert!(
        message.contains("Part of the tree could not be read"),
        "{message}"
    );
    assert_eq!(details["cause"], "unreadable_elements");
    assert_eq!(details["retryable"], false);
}
