use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, NotificationInfo};

use super::{dismiss_all_failures, dismiss_each_with};
#[cfg(target_os = "windows")]
use crate::notifications::actions::invoke_pattern;
use crate::notifications::actions::{missing_dismiss_error, read_control};

fn entry(index: usize, app: &str, title: &str) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: app.into(),
        title: title.into(),
        body: None,
        actions: Vec::new(),
    }
}

#[test]
fn a_filtered_dismiss_all_that_removes_its_target_leaves_another_apps_entry_unreported() {
    let captured = vec![entry(1, "App A", "cleared"), entry(2, "App B", "untouched")];
    let current = vec![entry(2, "App B", "untouched")];

    let failures = dismiss_all_failures(&captured, &current, &[]);

    assert_eq!(
        failures.len(),
        1,
        "App A's entry left, so only App B's untouched entry is a survivor"
    );
    assert!(failures[0].message.contains("#2"));
    assert_eq!(
        failures[0].disposition,
        DeliverySemantics::delivered_unverified()
    );
    assert!(
        !failures
            .iter()
            .any(|failure| failure.message.contains("#1")),
        "the entry that actually left must never appear as a failure"
    );
}

#[test]
fn a_recorded_invoke_error_names_its_own_reason_instead_of_the_generic_survivor_message() {
    let captured = vec![entry(1, "App A", "refused"), entry(2, "App B", "untouched")];
    let current = vec![entry(1, "App A", "refused"), entry(2, "App B", "untouched")];
    let invoke_errors = vec![(
        1,
        AdapterError::new(
            ErrorCode::ActionFailed,
            "the dismiss control could not be invoked",
        )
        .with_disposition(DeliverySemantics::not_delivered()),
    )];

    let failures = dismiss_all_failures(&captured, &current, &invoke_errors);

    assert_eq!(failures.len(), 2);
    let for_one = failures
        .iter()
        .find(|failure| failure.message.contains("#1"))
        .expect("index 1 is reported");
    assert!(
        for_one
            .message
            .contains("the dismiss control could not be invoked"),
        "the invoke's own error must survive into the failure line, not be discarded"
    );
    assert_eq!(
        for_one.disposition,
        DeliverySemantics::not_delivered(),
        "a failure recorded before anything was sent keeps its own disposition"
    );
    let for_two = failures
        .iter()
        .find(|failure| failure.message.contains("#2"))
        .expect("index 2 is reported");
    assert!(
        for_two.message.contains("still present"),
        "an entry with no recorded invoke error falls back to the generic survivor message"
    );
    assert_eq!(
        for_two.disposition,
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn an_invoke_error_recorded_for_an_entry_that_still_left_is_never_reported() {
    let captured = vec![
        entry(1, "App A", "left anyway"),
        entry(2, "App B", "stayed"),
    ];
    let current = vec![entry(2, "App B", "stayed")];
    let invoke_errors = vec![(
        1,
        AdapterError::new(ErrorCode::ActionFailed, "transient invoke error"),
    )];

    let failures = dismiss_all_failures(&captured, &current, &invoke_errors);

    assert_eq!(
        failures.len(),
        1,
        "the settle read proves index 1 left despite its invoke error, so it is dismissed, not failed"
    );
    assert!(!failures[0].message.contains("transient invoke error"));
    assert!(failures[0].message.contains("#2"));
}

fn unstamped_read_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "UI Automation could not search for a dismiss control",
    )
}

#[test]
fn a_filtered_dismiss_all_whose_pre_invoke_reads_fail_reports_nothing_delivered() {
    let captured = vec![entry(1, "App A", "first"), entry(2, "App B", "second")];
    let mut invoke_errors = Vec::new();
    let read_live = || {
        Ok(captured
            .iter()
            .cloned()
            .map(|info| (info, ()))
            .collect::<Vec<_>>())
    };
    let invoke = |_: &(), index: usize| {
        read_control::<()>(
            || Err(unstamped_read_error()),
            || missing_dismiss_error(index),
        )
    };

    dismiss_each_with(&captured, read_live, invoke, &mut invoke_errors)
        .expect("the batch completes with one recorded error per target");

    let failures = dismiss_all_failures(&captured, &captured, &invoke_errors);
    assert_eq!(failures.len(), 2);
    let dispositions: Vec<DeliverySemantics> =
        failures.iter().map(|failure| failure.disposition).collect();
    let merged = DeliverySemantics::merged(0, &dispositions);
    assert_eq!(merged, DeliverySemantics::not_delivered());
    assert_eq!(merged.retry(), agent_desktop_core::RetryDisposition::Safe);
}

#[test]
fn a_mid_batch_read_failure_after_an_invoke_is_delivered_unverified() {
    let captured = vec![entry(1, "App A", "first"), entry(2, "App B", "second")];
    let reads = std::cell::Cell::new(0);
    let mut invoke_errors = Vec::new();
    let read_live = || {
        reads.set(reads.get() + 1);
        if reads.get() == 1 {
            Ok(captured
                .iter()
                .cloned()
                .map(|info| (info, ()))
                .collect::<Vec<_>>())
        } else {
            Err(unstamped_read_error())
        }
    };
    let invoke = |_: &(), _index: usize| Ok(());

    let error = dismiss_each_with(&captured, read_live, invoke, &mut invoke_errors)
        .expect_err("the second listing fails");

    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[cfg(target_os = "windows")]
#[test]
fn a_pre_invoke_pattern_read_failure_keeps_the_batch_undispatched() {
    let captured = vec![entry(1, "App A", "first")];
    let mut invoke_errors = Vec::new();
    let read_live = || {
        Ok(captured
            .iter()
            .cloned()
            .map(|info| (info, ()))
            .collect::<Vec<_>>())
    };
    let invoke = |_: &(), _index: usize| {
        invoke_pattern(Err(uiautomation::Error::new(
            uiautomation::errors::ERR_INVALID_OBJECT,
            "the dismiss control exposes no Invoke pattern",
        )))
    };

    dismiss_each_with(&captured, read_live, invoke, &mut invoke_errors)
        .expect("the batch records the pre-invoke failure and completes");

    let failures = dismiss_all_failures(&captured, &captured, &invoke_errors);
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].disposition,
        DeliverySemantics::not_delivered(),
        "claiming the pattern sent nothing, so the batch never left not-delivered"
    );
    assert_eq!(
        failures[0].disposition.retry(),
        agent_desktop_core::RetryDisposition::Safe
    );
}
