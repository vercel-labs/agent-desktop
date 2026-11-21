use agent_desktop_core::{DeliverySemantics, ErrorCode, NotificationInfo};

use super::{
    action_changed_state, dismiss_survived_error, entry_gone, matching_count, same_identity,
    survivor_failure,
};

fn info(index: usize, app: &str, title: &str, body: Option<&str>) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: app.into(),
        title: title.into(),
        body: body.map(String::from),
        actions: Vec::new(),
    }
}

#[test]
fn identity_includes_app_title_and_body() {
    let original = info(1, "App A", "Reminder", Some("First"));

    assert!(same_identity(
        &original,
        &info(2, "App A", "Reminder", Some("First"))
    ));
    assert!(!same_identity(
        &original,
        &info(1, "App B", "Reminder", Some("First"))
    ));
    assert!(!same_identity(
        &original,
        &info(1, "App A", "Changed", Some("First"))
    ));
    assert!(!same_identity(
        &original,
        &info(1, "App A", "Reminder", Some("Second"))
    ));
}

#[test]
fn matching_count_counts_identity_matches_not_indices() {
    let target = info(1, "App A", "Reminder", Some("First"));
    let current = vec![
        info(1, "App A", "Reminder", Some("First")),
        info(2, "App B", "Other", None),
        info(3, "App A", "Reminder", Some("First")),
    ];

    assert_eq!(matching_count(&current, &target), 2);
}

#[test]
fn an_entry_is_gone_only_when_no_identity_match_remains() {
    let target = info(1, "App A", "Reminder", Some("First"));

    assert!(entry_gone(&target, &[info(1, "App B", "Other", None)]));
    assert!(!entry_gone(
        &target,
        &[info(3, "App A", "Reminder", Some("First"))]
    ));
    assert!(entry_gone(&target, &[]));
}

#[test]
fn a_survivor_without_a_recorded_invoke_error_is_delivered_unverified() {
    let error = survivor_failure();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(error.message.contains("still present"));
    assert!(
        !error.message.contains("Secret"),
        "the survivor line is built from the index alone and must never carry entry content"
    );
}

#[test]
fn an_action_that_removes_the_entry_counts_as_a_state_change() {
    let original = info(1, "App A", "Reminder", None);

    assert!(action_changed_state(&original, &[]));
}

#[test]
fn an_action_that_changes_the_entry_counts_as_a_state_change() {
    let original = info(1, "App A", "Reminder", Some("before"));
    let after = vec![info(1, "App A", "Reminder", Some("after"))];

    assert!(action_changed_state(&original, &after));
}

#[test]
fn an_unacknowledged_entry_left_unchanged_is_not_reported_as_a_change() {
    let original = info(1, "App A", "Reminder", Some("same"));
    let after = vec![info(1, "App A", "Reminder", Some("same"))];

    assert!(!action_changed_state(&original, &after));
}

#[test]
fn a_surviving_dismiss_is_action_failed_and_delivered_unverified() {
    let error = dismiss_survived_error(3);

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(error.message.contains("index 3"));
    assert!(
        !error.is_explicitly_retryable(),
        "the disposition is the retry contract: a delivered-unverified envelope serializes \
         retry-unsafe, so a contradictory retryable detail stamp must not ride beside it"
    );
}
