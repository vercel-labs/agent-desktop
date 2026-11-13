use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, NotificationInfo};

fn notification_entry(index: usize, title: &str) -> crate::notifications::list::NotificationEntry {
    crate::notifications::list::NotificationEntry {
        info: NotificationInfo {
            index,
            app_name: "Mail".to_owned(),
            title: title.to_owned(),
            body: None,
            actions: Vec::new(),
        },
        element: crate::tree::AXElement(std::ptr::null_mut()),
    }
}

#[test]
fn an_entry_absent_from_the_fresh_listing_counts_as_dismissed_without_an_attempt() {
    let original = notification_entry(2, "Standup");
    let current = vec![notification_entry(1, "Other")];
    let mut attempted = false;

    let result = super::dismiss_turn(&original, &current, |_, _| {
        attempted = true;
        Ok(())
    });

    assert!(result.is_ok());
    assert!(!attempted);
}

#[test]
fn a_held_entry_that_dismisses_counts_as_dismissed() {
    let original = notification_entry(2, "Standup");
    let current = vec![notification_entry(2, "Standup")];

    let result = super::dismiss_turn(&original, &current, |_, _| Ok(()));

    assert!(result.is_ok());
}

#[test]
fn a_held_entry_whose_attempt_fails_reports_the_captured_index() {
    let original = notification_entry(2, "Standup");
    let current = vec![notification_entry(2, "Standup")];

    let result = super::dismiss_turn(&original, &current, |_, _| {
        Err(AdapterError::new(ErrorCode::ActionFailed, "still there"))
    });

    let error = result.expect_err("a failed attempt is a failure line");
    assert_eq!(error.message, "#2: still there");
}

fn listing_failure() -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, "listing failed")
}

#[test]
fn a_listing_failure_after_a_dismissal_returns_the_partial_result() {
    let entries = vec![
        notification_entry(1, "First"),
        notification_entry(2, "Second"),
    ];
    let mut listings = 0;

    let (dismissed, failures) = super::dismiss_captured(
        &entries,
        || {
            listings += 1;
            if listings == 1 {
                Ok(vec![notification_entry(2, "Second")])
            } else {
                Err(listing_failure())
            }
        },
        |_, _| Ok(()),
    )
    .expect("a dispatched dismissal keeps the partial result");

    let dispositions: Vec<_> = failures.iter().map(|failure| failure.disposition).collect();
    assert_eq!(dismissed.len(), 1);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].message.contains("listing failed"));
    assert_ne!(
        DeliverySemantics::merged(dismissed.len(), &dispositions),
        DeliverySemantics::not_delivered()
    );
}

#[test]
fn a_listing_failure_before_any_dispatch_is_the_command_error() {
    let entries = vec![notification_entry(1, "First")];

    let result = super::dismiss_captured(&entries, || Err(listing_failure()), |_, _| Ok(()));

    assert_eq!(
        result.expect_err("no dispatch happened").message,
        "listing failed"
    );
}

#[test]
fn a_listing_failure_after_only_absent_entries_does_not_claim_a_send() {
    let entries = vec![
        notification_entry(1, "First"),
        notification_entry(2, "Second"),
    ];
    let mut listings = 0;

    let (dismissed, failures) = super::dismiss_captured(
        &entries,
        || {
            listings += 1;
            if listings == 1 {
                Ok(Vec::new())
            } else {
                Err(listing_failure())
            }
        },
        |_, _| Ok(()),
    )
    .expect("entries already gone keep the partial result");

    let dispositions: Vec<_> = failures.iter().map(|failure| failure.disposition).collect();
    assert_eq!(dismissed.len(), 1);
    assert_eq!(dispositions, vec![DeliverySemantics::not_delivered()]);
    assert_ne!(
        DeliverySemantics::merged(dismissed.len(), &dispositions),
        DeliverySemantics::delivered_unverified()
    );
}
