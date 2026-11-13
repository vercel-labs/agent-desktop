//! Pins the shared resolver-error payload byte-for-byte against the two
//! platform copies these constructors replaced
//! (`crates/windows/src/tree/resolve/resolve_search.rs::identity_unknown_error`,
//! `crates/macos/src/tree/resolve/resolve_errors.rs::identity_unknown`, and both
//! platforms' `mark_deadline_elapsed`, captured before deletion). Renaming or
//! dropping a key here is a behaviour change for both adapters at once.

use super::*;

fn entry_with_role(role: &str) -> RefEntry {
    crate::adapter::minimal_ref_entry(role, None)
}

#[test]
fn identity_unknown_error_pins_the_shared_payload() {
    let entry = entry_with_role("button");
    let error = identity_unknown_error(&entry);

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert_eq!(
        error.message,
        "Strict resolution could not determine candidate identity from the live accessibility evidence"
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Retry after the target application finishes updating its accessibility tree")
    );
    assert_eq!(
        error.details,
        Some(json!({
            "kind": "resolution_identity_unknown",
            "role": "button",
            "complete": false,
            "retryable": true,
        }))
    );
    assert!(error.is_explicitly_retryable());
    assert!(error.permits_retry_by_default());
}

#[test]
fn mark_deadline_elapsed_stamps_an_object_details_payload() {
    let error = AdapterError::new(ErrorCode::AppUnresponsive, "incomplete").with_details(json!({
        "kind": "resolution_identity_unknown",
        "complete": false,
        "retryable": true,
    }));

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(
        stamped.details,
        Some(json!({
            "kind": "resolution_identity_unknown",
            "complete": false,
            "retryable": true,
            "deadline_elapsed": true,
        }))
    );
}

#[test]
fn mark_deadline_elapsed_wraps_a_non_object_details_payload() {
    let error =
        AdapterError::new(ErrorCode::Timeout, "bare").with_details(json!("evidence-string"));

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(
        stamped.details,
        Some(json!({
            "evidence": "evidence-string",
            "deadline_elapsed": true,
        }))
    );
}

#[test]
fn mark_deadline_elapsed_fills_missing_details() {
    let error = AdapterError::new(ErrorCode::Timeout, "bare");

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(stamped.details, Some(json!({ "deadline_elapsed": true })));
}

#[test]
fn a_stale_element_token_is_settled_and_carries_no_retry() {
    let entry = entry_with_role("button");
    let error = element_token_stale(&entry, "dead", None);

    assert_eq!(error.code, ErrorCode::StaleRef);
    let details = error.details.as_ref().expect("details");
    assert_eq!(details["kind"], json!("retained_object_stale"));
    assert_eq!(details["retryable"], json!(false));
    assert_eq!(details["reason"], json!("dead"));
    assert!(details.get("depth").is_none());
    assert!(!error.permits_retry_by_default());
}

#[test]
fn a_stale_element_token_keeps_the_depth_of_the_broken_link() {
    let entry = entry_with_role("button");
    let error = element_token_stale(&entry, "chain", Some(2));

    let details = error.details.as_ref().expect("details");
    assert_eq!(details["depth"], json!(2));
    assert_eq!(details["retryable"], json!(false));
}

#[test]
fn every_settled_ref_refusal_refuses_a_retry() {
    let entry = entry_with_role("button");
    let refusals = [
        ("saved_ref_absent", absent(&entry)),
        ("saved_ref_ambiguous", ambiguous_live_match(&entry)),
        ("identity_not_unique", identity_not_unique(&entry)),
        ("element_token_required", element_token_required(&entry)),
        (
            "retained_object_stale",
            element_token_stale(&entry, "dead", None),
        ),
    ];

    for (kind, error) in refusals {
        let retryable = error
            .details
            .as_ref()
            .and_then(|details| details.get("retryable"));
        assert_eq!(retryable, Some(&json!(false)), "{kind}");
        assert!(!error.permits_retry_by_default(), "{kind}");
    }
}
