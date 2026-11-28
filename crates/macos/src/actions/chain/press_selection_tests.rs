use std::cell::Cell;

use agent_desktop_core::{AdapterError, DeliveryObservation, DeliveryOutcome, DeliverySemantics};

use super::press_then_select;

const SENT: DeliveryOutcome = DeliveryOutcome::Delivered(DeliveryObservation::Unverified);
const SEEN: DeliveryOutcome = DeliveryOutcome::Delivered(DeliveryObservation::Verified);

fn press_with(
    selection: Option<bool>,
    container_selection: DeliveryOutcome,
) -> (DeliveryOutcome, bool) {
    let fallback_ran = Cell::new(false);
    let outcome = press_then_select(
        || Ok(SENT),
        || selection,
        || {
            fallback_ran.set(true);
            Ok(container_selection)
        },
    )
    .unwrap();
    (outcome, fallback_ran.get())
}

#[test]
fn a_press_that_changed_nothing_runs_the_container_selection_and_is_verified_by_it() {
    assert_eq!(press_with(Some(false), SEEN), (SEEN, true));
}

#[test]
fn a_container_selection_that_sends_nothing_leaves_the_press_unverified() {
    assert_eq!(
        press_with(Some(false), DeliveryOutcome::NotDelivered),
        (SENT, true)
    );
    assert_eq!(press_with(Some(false), SENT), (SENT, true));
}

#[test]
fn a_press_that_selected_the_target_is_verified_without_the_fallback() {
    assert_eq!(press_with(Some(true), SEEN), (SEEN, false));
}

#[test]
fn an_element_without_a_selection_keeps_its_unverified_press_and_skips_the_fallback() {
    assert_eq!(press_with(None, SEEN), (SENT, false));
}

#[test]
fn a_press_that_settled_the_action_never_reads_the_selection_or_selects() {
    for pressed in [
        DeliveryOutcome::NotDelivered,
        DeliveryOutcome::Satisfied,
        SEEN,
    ] {
        let outcome = press_then_select(
            || Ok(pressed),
            || panic!("the selection was read after a settled press"),
            || panic!("the container was selected after a settled press"),
        )
        .unwrap();
        assert_eq!(outcome, pressed);
    }
}

#[test]
fn a_container_selection_failing_after_the_press_keeps_the_press_on_record() {
    let error = press_then_select(
        || Ok(SENT),
        || Some(false),
        || {
            Err(AdapterError::internal("selection write failed")
                .with_disposition(DeliverySemantics::not_delivered()))
        },
    )
    .unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
