use super::{ToggleGoal, ToggleKind, drive_toggle};
use crate::actions::chain::DeliveryOutcome;
use crate::system::fixture::test_time::deadline;
use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use std::cell::Cell;

fn drive(
    goal: ToggleGoal,
    start: ToggleKind,
    cycle: &'static [ToggleKind],
    stuck: bool,
) -> (DeliveryOutcome, u8) {
    let state = Cell::new(start);
    let flips = Cell::new(0u8);
    let outcome = drive_toggle(
        goal,
        deadline(5_000),
        &mut || Some(state.get()),
        &mut || {
            if !stuck {
                let at = cycle
                    .iter()
                    .position(|kind| *kind == state.get())
                    .unwrap_or(0);
                state.set(cycle[(at + 1) % cycle.len()]);
            }
            flips.set(flips.get() + 1);
            Ok(true)
        },
    )
    .expect("drive");
    (outcome, flips.get())
}

const TWO_STATE: &[ToggleKind] = &[ToggleKind::Off, ToggleKind::On];
const TRI_STATE: &[ToggleKind] = &[ToggleKind::Off, ToggleKind::On, ToggleKind::Indeterminate];

#[test]
fn an_observed_flip_is_verified_after_one_delivery() {
    let (outcome, flips) = drive(ToggleGoal::Flip, ToggleKind::Off, TWO_STATE, false);
    assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
    assert_eq!(flips, 1);
}

#[test]
fn a_flip_the_state_never_shows_is_unverified_and_not_resent() {
    let (outcome, flips) = drive(ToggleGoal::Checked(true), ToggleKind::Off, TWO_STATE, true);
    assert_eq!(outcome, DeliveryOutcome::DeliveredUnverified);
    assert_eq!(flips, 1);
}

#[test]
fn check_from_indeterminate_flips_twice_and_verifies() {
    let (outcome, flips) = drive(
        ToggleGoal::Checked(true),
        ToggleKind::Indeterminate,
        TRI_STATE,
        false,
    );
    assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
    assert_eq!(flips, 2);
}

#[test]
fn a_mechanism_that_is_absent_reports_not_delivered() {
    let outcome = drive_toggle(
        ToggleGoal::Flip,
        deadline(5_000),
        &mut || Some(ToggleKind::Off),
        &mut || Ok(false),
    )
    .expect("drive");
    assert_eq!(outcome, DeliveryOutcome::NotDelivered);
}

#[test]
fn a_flip_from_an_unreadable_state_is_not_verified() {
    let reads = Cell::new(0u8);
    let outcome = drive_toggle(
        ToggleGoal::Flip,
        deadline(5_000),
        &mut || {
            reads.set(reads.get() + 1);
            (reads.get() > 1).then_some(ToggleKind::Off)
        },
        &mut || Ok(true),
    )
    .expect("drive");
    assert_eq!(outcome, DeliveryOutcome::DeliveredUnverified);
}

#[test]
fn an_error_on_the_second_flip_keeps_the_first_delivery() {
    let state = Cell::new(ToggleKind::Indeterminate);
    let flips = Cell::new(0u8);
    let error = drive_toggle(
        ToggleGoal::Checked(true),
        deadline(5_000),
        &mut || Some(state.get()),
        &mut || {
            flips.set(flips.get() + 1);
            if flips.get() == 1 {
                state.set(ToggleKind::Off);
                return Ok(true);
            }
            Err(AdapterError::new(ErrorCode::ActionFailed, "not enabled")
                .with_disposition(DeliverySemantics::not_delivered()))
        },
    )
    .expect_err("second flip fails");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
