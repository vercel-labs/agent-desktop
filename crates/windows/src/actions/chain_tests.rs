use super::{ChainRung, DeliveryOutcome, build_step, execute_chain, legacy_refusal_is_absent};
use crate::system::fixture::test_time::deadline;
use agent_desktop_core::{
    ActionStepOutcome, AdapterError, DeliverySemantics, ErrorCode, InteractionPolicy, StepMechanism,
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
        deadline(5_000),
        "retry",
        policy,
        &mut [
            ChainRung {
                label: LEGACY,
                available: legacy.0,
                requires_headed: false,
                run: &mut legacy_run,
            },
            ChainRung {
                label: PATTERN,
                available: pattern.0,
                requires_headed: true,
                run: &mut pattern_run,
            },
        ],
    )
}

#[test]
fn headless_delivers_through_the_background_rung_and_never_runs_the_pattern() {
    let calls = Cell::new((0, 0));
    let steps = run_two(
        InteractionPolicy::headless(),
        (true, DeliveryOutcome::DeliveredVerified),
        (true, DeliveryOutcome::DeliveredVerified),
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
        (false, DeliveryOutcome::DeliveredVerified),
        (true, DeliveryOutcome::DeliveredVerified),
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
        (true, DeliveryOutcome::DeliveredVerified),
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
        (true, DeliveryOutcome::DeliveredUnverified),
        &calls,
    )
    .expect("pattern delivers");
    assert_eq!(calls.get(), (1, 1));
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Skipped));
    assert!(matches!(steps[1].outcome, ActionStepOutcome::Succeeded));
    assert_eq!(steps[1].verified(), Some(false));
}

#[test]
fn nothing_offered_exhausts_not_delivered_with_the_suggestion() {
    let calls = Cell::new((0, 0));
    let error = run_two(
        InteractionPolicy::headed(),
        (false, DeliveryOutcome::DeliveredVerified),
        (false, DeliveryOutcome::DeliveredVerified),
        &calls,
    )
    .expect_err("nothing offered");
    assert_eq!(calls.get(), (0, 0));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.suggestion.as_deref(), Some("retry"));
}

#[test]
fn genuine_error_aborts_before_any_later_rung() {
    let later = Cell::new(0u8);
    let mut failing = || Err(AdapterError::new(ErrorCode::StaleRef, "gone"));
    let mut second = || {
        later.set(later.get() + 1);
        Ok(DeliveryOutcome::DeliveredVerified)
    };
    let error = execute_chain(
        deadline(5_000),
        "retry",
        InteractionPolicy::headed(),
        &mut [
            ChainRung {
                label: LEGACY,
                available: true,
                requires_headed: false,
                run: &mut failing,
            },
            ChainRung {
                label: PATTERN,
                available: true,
                requires_headed: true,
                run: &mut second,
            },
        ],
    )
    .expect_err("error aborts");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(later.get(), 0);
}

#[test]
fn build_step_records_delivery_and_verification_separately() {
    let cases = [
        (DeliveryOutcome::NotDelivered, false, None),
        (DeliveryOutcome::SatisfiedNoDelivery, false, Some(true)),
        (DeliveryOutcome::DeliveredUnverified, true, Some(false)),
        (DeliveryOutcome::DeliveredVerified, true, Some(true)),
        (DeliveryOutcome::from_observation(None), true, None),
    ];
    for (outcome, succeeded, verified) in cases {
        let step = build_step(LEGACY, outcome);
        assert_eq!(step.mechanism(), Some(StepMechanism::SemanticApi));
        assert_eq!(
            matches!(step.outcome, ActionStepOutcome::Succeeded),
            succeeded
        );
        assert_eq!(step.verified(), verified);
    }
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
