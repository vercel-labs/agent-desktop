//! Behavior of the shared live read through the closures
//! `read_live_element_core` is generic over: the corroborator, the property
//! read and the label read. Each stands in for a condition a real element
//! cannot be made to produce on demand. The element itself is only a token the
//! closures receive, so the desktop root serves and no fixture window is
//! needed.

use super::imp::read_live_element_core;
use super::{ElementProperties, stale_reader_error};
use crate::system::runtime::hresult::UIA_E_ELEMENTNOTAVAILABLE;
use crate::system::test_support::apartment::ensure_test_apartment;
use crate::system::test_support::test_time::deadline;
use crate::tree::automation::{UiaFailure, automation_client, uia_error, uia_failure_error};
use crate::tree::element::UIAElement;
use crate::tree::name_evidence::LabelOutcome;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::properties::{read_live, read_live_bounded};
use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use std::cell::Cell;
use std::time::Duration;

fn desktop_root() -> UIAElement {
    ensure_test_apartment();
    let client = automation_client().expect("a UIA client is available");
    UIAElement::from(
        client
            .get_root_element()
            .map_err(|error| uia_error(&error, "read the desktop root"))
            .expect("the desktop root resolves"),
    )
}

fn unlabelled(_: &UIAElement) -> LabelOutcome {
    LabelOutcome::Unlabelled
}

fn alive(_: &UIAElement) -> Result<(), AdapterError> {
    Ok(())
}

fn nothing_read(_: &UIAElement) -> (ElementProperties, Vec<AdapterError>) {
    (ElementProperties::from_reads(Vec::new()), Vec::new())
}

fn details_flag(error: &AdapterError, key: &str) -> Option<bool> {
    error
        .details
        .as_ref()
        .and_then(|details| details.get(key))
        .and_then(serde_json::Value::as_bool)
}

#[test]
fn a_process_that_dies_during_the_read_settles_stale_not_ok() {
    let element = desktop_root();
    let calls = Cell::new(0u32);
    let corroborate = |_: &UIAElement| {
        let already_called = calls.get();
        calls.set(already_called + 1);
        if already_called == 0 {
            Ok(())
        } else {
            Err(stale_reader_error())
        }
    };

    let outcome = read_live_element_core(
        &element,
        deadline(10_000),
        false,
        corroborate,
        read_live,
        unlabelled,
    );

    let error = outcome
        .err()
        .expect("a token that goes dead mid-call must not read Ok");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        calls.get(),
        2,
        "the pre-read and the post-read corroboration both run, so the read is not trusted alone"
    );
}

#[test]
fn a_vanished_resolved_target_settles_stale_complete_not_retryable_unresponsive() {
    let element = desktop_root();
    let vanished_read = |_: &UIAElement| {
        let error = uia_failure_error(
            UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE),
            "read an element property",
        );
        (ElementProperties::from_reads(Vec::new()), vec![error])
    };

    let outcome = read_live_element_core(
        &element,
        deadline(10_000),
        false,
        alive,
        vanished_read,
        unlabelled,
    );

    let error = outcome
        .err()
        .expect("a vanished resolved target must not read as complete");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(details_flag(&error, "complete"), Some(true));
    assert!(!error.is_transient_incomplete_read());
}

#[test]
fn a_bundle_whose_essential_slots_are_unknown_fails_retryable_never_partial() {
    let element = desktop_root();

    let outcome = read_live_element_core(
        &element,
        deadline(10_000),
        false,
        alive,
        nothing_read,
        unlabelled,
    );

    let error = outcome
        .err()
        .expect("a bundle with no essential slot read must not answer as a live read");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert!(error.is_explicitly_retryable());
    assert_eq!(details_flag(&error, "complete"), Some(false));
    assert_eq!(details_flag(&error, "retryable"), Some(true));
}

#[test]
fn a_process_that_is_dead_before_the_read_never_pays_for_the_read() {
    let element = desktop_root();
    let reads = Cell::new(0u32);
    let counted_read = |element: &UIAElement| {
        reads.set(reads.get() + 1);
        nothing_read(element)
    };

    let outcome = read_live_element_core(
        &element,
        deadline(10_000),
        false,
        |_: &UIAElement| Err(stale_reader_error()),
        counted_read,
        unlabelled,
    );

    assert_eq!(
        outcome.err().map(|error| error.code),
        Some(ErrorCode::StaleRef)
    );
    assert_eq!(reads.get(), 0);
}

#[test]
fn a_read_that_spent_the_deadline_skips_the_label_read_and_a_read_with_budget_does_not() {
    let element = desktop_root();
    let unhurried_labels = Cell::new(0u32);
    let spent_labels = Cell::new(0u32);
    let counted = |calls: &Cell<u32>| {
        calls.set(calls.get() + 1);
        LabelOutcome::Unlabelled
    };

    let _ = read_live_element_core(
        &element,
        deadline(10_000),
        false,
        alive,
        nothing_read,
        |_: &UIAElement| counted(&unhurried_labels),
    );

    let spent_budget = Deadline::after(300).expect("a short deadline builds");
    let read_past_the_budget = |element: &UIAElement| {
        std::thread::sleep(spent_budget.remaining() + Duration::from_millis(50));
        nothing_read(element)
    };
    let _ = read_live_element_core(
        &element,
        spent_budget,
        false,
        alive,
        read_past_the_budget,
        |_: &UIAElement| counted(&spent_labels),
    );

    assert_eq!(unhurried_labels.get(), 1);
    assert_eq!(spent_labels.get(), 0);
}

#[test]
fn a_minimized_owner_stamps_the_subtree_offscreen_and_a_restored_owner_does_not() {
    let element = desktop_root();
    let (raw, _) = read_live_bounded(&element, deadline(10_000));

    let minimized = read_live_element_core(
        &element,
        deadline(10_000),
        true,
        alive,
        read_live,
        unlabelled,
    )
    .expect("a live read of the desktop root completes");
    let restored = read_live_element_core(
        &element,
        deadline(10_000),
        false,
        alive,
        read_live,
        unlabelled,
    )
    .expect("a live read of the desktop root completes");

    assert_eq!(
        minimized.properties.get(TreeProperty::IsOffscreen).flag(),
        Some(true)
    );
    assert_eq!(
        restored.properties.get(TreeProperty::IsOffscreen).flag(),
        raw.get(TreeProperty::IsOffscreen).flag()
    );
}
