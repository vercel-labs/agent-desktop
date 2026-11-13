use super::{legacy_refusal_is_absent, offered, rung};
use crate::system::test_support::test_time::deadline;
use agent_desktop_core::{
    ActionStepOutcome, AdapterError, DeliveryObservation, DeliveryOutcome, DeliverySemantics,
    ErrorCode, InteractionPolicy, execute_chain,
};
use std::cell::Cell;

const LEGACY: &str = "LegacyIAccessible.DoDefaultAction";
const PATTERN: &str = "InvokePattern.Invoke";

fn run_two(
    policy: InteractionPolicy,
    legacy: (bool, DeliveryOutcome),
    pattern: (bool, DeliveryOutcome),
    calls: &Cell<(u8, u8)>,
) -> Result<Vec<agent_desktop_core::ActionStep>, AdapterError> {
    let mut legacy_run = || {
        calls.set((calls.get().0 + 1, calls.get().1));
        Ok(legacy.1)
    };
    let mut pattern_run = || {
        calls.set((calls.get().0, calls.get().1 + 1));
        Ok(pattern.1)
    };
    execute_chain(
        &mut offered([
            rung(LEGACY, legacy.0, false, &mut legacy_run),
            rung(PATTERN, pattern.0, true, &mut pattern_run),
        ]),
        policy,
        deadline(5_000),
        "retry",
    )
}

#[test]
fn headless_delivers_through_the_background_rung_and_never_runs_the_pattern() {
    let calls = Cell::new((0, 0));
    let steps = run_two(
        InteractionPolicy::headless(),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        &calls,
    )
    .expect("legacy delivers");
    assert_eq!(calls.get(), (1, 0));
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].verified(), Some(true));
}

#[test]
fn headless_with_only_a_foreground_rung_is_policy_denied_without_running_it() {
    let calls = Cell::new((0, 0));
    let error = run_two(
        InteractionPolicy::headless(),
        (
            false,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        &calls,
    )
    .expect_err("only the pattern is offered");
    assert_eq!(calls.get(), (0, 0));
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["foreground_effect"], true);
    assert_eq!(details["mechanisms"][0], PATTERN);
}

#[test]
fn headless_background_rung_that_does_not_deliver_ends_in_policy_denied() {
    let calls = Cell::new((0, 0));
    let error = run_two(
        InteractionPolicy::headless(),
        (true, DeliveryOutcome::NotDelivered),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        &calls,
    )
    .expect_err("pattern withheld");
    assert_eq!(calls.get(), (1, 0));
    assert_eq!(error.code, ErrorCode::PolicyDenied);
}

#[test]
fn headed_falls_through_to_the_pattern_after_a_clean_miss() {
    let calls = Cell::new((0, 0));
    let steps = run_two(
        InteractionPolicy::headed(),
        (true, DeliveryOutcome::NotDelivered),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Unverified),
        ),
        &calls,
    )
    .expect("pattern delivers");
    assert_eq!(calls.get(), (1, 1));
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Skipped));
    assert!(matches!(steps[1].outcome, ActionStepOutcome::Succeeded));
    assert_eq!(steps[1].verified(), Some(false));
}

#[test]
fn headed_tries_the_pattern_after_a_legacy_write_that_had_no_effect() {
    let calls = Cell::new((0, 0));
    let steps = run_two(
        InteractionPolicy::headed(),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::NoEffect),
        ),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        &calls,
    )
    .expect("pattern delivers");
    assert_eq!(calls.get(), (1, 1));
    assert_eq!(steps[0].verified(), Some(false));
    assert_eq!(steps[1].verified(), Some(true));
}

#[test]
fn nothing_offered_exhausts_not_delivered_with_the_suggestion() {
    let calls = Cell::new((0, 0));
    let error = run_two(
        InteractionPolicy::headed(),
        (
            false,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        (
            false,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        &calls,
    )
    .expect_err("nothing offered");
    assert_eq!(calls.get(), (0, 0));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.suggestion.as_deref(), Some("retry"));
}

#[test]
fn a_legacy_not_enabled_refusal_is_an_absent_route_only_on_an_enabled_element() {
    let not_enabled = 0x8004_0200_u32 as i32;
    assert!(legacy_refusal_is_absent(not_enabled, Some(true)));
    assert!(!legacy_refusal_is_absent(not_enabled, Some(false)));
    assert!(!legacy_refusal_is_absent(not_enabled, None));
    assert!(!legacy_refusal_is_absent(
        0x8004_0201_u32 as i32,
        Some(true)
    ));
}
