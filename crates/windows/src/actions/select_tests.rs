use super::{
    EXPAND_LABEL, ExpandKind, SELECT_SUGGESTION, annotate_container, collapse_container, run_expand,
};
use crate::system::test_support::test_time::deadline;
use agent_desktop_core::{
    AdapterError, DeliveryObservation, DeliveryOutcome, DeliverySemantics, ErrorCode,
    InteractionPolicy, RetryDisposition,
};
use std::cell::Cell;

fn expanded() -> Result<DeliveryOutcome, AdapterError> {
    Ok(DeliveryOutcome::Delivered(DeliveryObservation::Unverified))
}

#[test]
fn a_headless_select_on_a_collapsed_container_is_refused_without_expanding() {
    let calls = Cell::new(0);
    let mut expand = || {
        calls.set(calls.get() + 1);
        expanded()
    };

    let error = run_expand(&mut expand, InteractionPolicy::headless(), deadline(5_000))
        .expect_err("headless refuses the expand");

    assert_eq!(calls.get(), 0);
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["mechanisms"][0], EXPAND_LABEL);
}

#[test]
fn a_policy_that_allows_focus_steal_expands_the_container() {
    for policy in [
        InteractionPolicy::focus_fallback(),
        InteractionPolicy::headed(),
    ] {
        let calls = Cell::new(0);
        let mut expand = || {
            calls.set(calls.get() + 1);
            expanded()
        };

        run_expand(&mut expand, policy, deadline(5_000)).expect("the expand is allowed");

        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn an_expand_that_sent_nothing_is_not_delivered() {
    let mut expand = || Ok(DeliveryOutcome::NotDelivered);

    let error = run_expand(&mut expand, InteractionPolicy::headed(), deadline(5_000))
        .expect_err("nothing expanded");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.suggestion.as_deref(), Some(SELECT_SUGGESTION));
}

#[test]
fn an_expand_failure_reaches_the_caller_with_its_own_code() {
    for policy in [
        InteractionPolicy::focus_fallback(),
        InteractionPolicy::headed(),
    ] {
        let mut expand = || {
            Err(
                AdapterError::new(ErrorCode::StaleRef, "the container vanished")
                    .with_disposition(DeliverySemantics::not_delivered()),
            )
        };

        let error = run_expand(&mut expand, policy, deadline(5_000))
            .expect_err("the expand failure surfaces");

        assert_eq!(error.code, ErrorCode::StaleRef);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}

#[test]
fn a_failed_select_after_an_expand_says_what_the_collapse_read_proved() {
    for (collapsed, expected) in [
        (Some(true), serde_json::json!(true)),
        (Some(false), serde_json::json!(false)),
        (None, serde_json::json!(null)),
    ] {
        let error = annotate_container(
            AdapterError::new(ErrorCode::ElementNotFound, "no item")
                .with_disposition(DeliverySemantics::not_delivered()),
            collapsed,
        );

        assert_eq!(error.code, ErrorCode::ElementNotFound);
        let details = error.details.expect("details");
        assert_eq!(details["container_expanded"], true);
        assert_eq!(details["container_collapsed"], expected);
    }
}

#[test]
fn an_expanded_container_that_did_not_close_is_delivered_unverified_with_unsafe_retry() {
    for collapsed in [Some(false), None] {
        let error = annotate_container(
            AdapterError::new(ErrorCode::ElementNotFound, "no item")
                .with_disposition(DeliverySemantics::not_delivered()),
            collapsed,
        );

        assert_eq!(error.code, ErrorCode::ElementNotFound);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
    }
}

#[test]
fn a_container_proven_collapsed_again_keeps_the_select_errors_disposition() {
    let error = annotate_container(
        AdapterError::new(ErrorCode::ElementNotFound, "no item")
            .with_disposition(DeliverySemantics::not_delivered()),
        Some(true),
    );

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
}

#[test]
fn the_container_annotation_keeps_the_details_the_error_carried() {
    let error = AdapterError::ambiguous_target("two items")
        .with_details(serde_json::json!({ "kind": "ambiguous_select_value" }));

    let details = annotate_container(error, Some(false))
        .details
        .expect("details");

    assert_eq!(details["kind"], "ambiguous_select_value");
    assert_eq!(details["container_expanded"], true);
    assert_eq!(details["container_collapsed"], false);
}

#[test]
fn the_collapse_report_comes_from_the_state_read_not_the_write_ack() {
    let mut collapse = || true;
    let mut read = || Some(ExpandKind::Expanded);

    let report = collapse_container(&mut collapse, &mut read, deadline(60));

    assert_eq!(report, Some(false));
}

#[test]
fn a_collapse_the_state_read_confirms_reports_collapsed() {
    let reads = Cell::new(0);
    let mut collapse = || true;
    let mut read = || {
        reads.set(reads.get() + 1);
        Some(ExpandKind::Collapsed)
    };

    let report = collapse_container(&mut collapse, &mut read, deadline(5_000));

    assert_eq!(report, Some(true));
    assert_eq!(reads.get(), 1);
}

#[test]
fn a_container_that_collapses_late_in_the_poll_reports_collapsed() {
    let reads = Cell::new(0);
    let mut collapse = || true;
    let mut read = || {
        let read_count = reads.get();
        reads.set(read_count + 1);
        Some(if read_count < 2 {
            ExpandKind::Expanded
        } else {
            ExpandKind::Collapsed
        })
    };

    let report = collapse_container(&mut collapse, &mut read, deadline(5_000));

    assert_eq!(report, Some(true));
    assert_eq!(reads.get(), 3);
}

#[test]
fn a_state_read_that_never_answers_is_unknown_not_collapsed() {
    let mut collapse = || true;
    let mut read = || None;

    let report = collapse_container(&mut collapse, &mut read, deadline(60));

    assert_eq!(report, None);
}

#[test]
fn the_collapse_attempt_still_runs_when_the_read_has_no_budget() {
    let collapse_calls = Cell::new(0);
    let mut collapse = || {
        collapse_calls.set(collapse_calls.get() + 1);
        true
    };
    let reads = Cell::new(0);
    let mut read = || {
        reads.set(reads.get() + 1);
        Some(ExpandKind::Expanded)
    };

    let report = collapse_container(&mut collapse, &mut read, deadline(0));

    assert_eq!(report, Some(false));
    assert_eq!(collapse_calls.get(), 1);
    assert_eq!(reads.get(), 1);
}
