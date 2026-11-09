use super::{foreground_route_allowed, legacy_refusal_is_absent, offered, poll_until, rung};
use crate::system::test_support::test_time::deadline;
use agent_desktop_core::{
    ActionStepOutcome, AdapterError, DeliveryObservation, DeliveryOutcome, DeliverySemantics,
    ErrorCode, InteractionPolicy, execute_chain,
};
use std::cell::Cell;
use std::time::Duration;

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
            rung(PATTERN, pattern.0, true, &mut pattern_run),
            rung(LEGACY, legacy.0, false, &mut legacy_run),
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
fn headed_never_runs_legacy_after_an_unverified_pattern_delivery() {
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
    .expect("the pattern delivery ends the chain");
    assert_eq!(calls.get(), (0, 1));
    assert_eq!(steps.len(), 1);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Succeeded));
    assert_eq!(steps[0].verified(), Some(false));
}

#[test]
fn headed_runs_the_verifiable_pattern_first_and_never_the_legacy_write() {
    let calls = Cell::new((0, 0));
    let steps = run_two(
        InteractionPolicy::headed(),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Unverified),
        ),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        &calls,
    )
    .expect("the pattern write ends the chain");
    assert_eq!(calls.get(), (0, 1));
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].verified(), Some(true));
}

#[test]
fn headed_falls_back_to_legacy_when_the_pattern_delivers_nothing() {
    let calls = Cell::new((0, 0));
    let steps = run_two(
        InteractionPolicy::headed(),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        (true, DeliveryOutcome::NotDelivered),
        &calls,
    )
    .expect("legacy delivers after the pattern misses");
    assert_eq!(calls.get(), (1, 1));
    assert_eq!(steps.len(), 2);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Skipped));
    assert_eq!(steps[1].verified(), Some(true));
}

#[test]
fn headless_keeps_an_acknowledged_legacy_write_unverified() {
    let calls = Cell::new((0, 0));
    let steps = run_two(
        InteractionPolicy::headless(),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Unverified),
        ),
        (
            true,
            DeliveryOutcome::Delivered(DeliveryObservation::Verified),
        ),
        &calls,
    )
    .expect("the legacy write ends the chain");
    assert_eq!(calls.get(), (1, 0));
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].verified(), Some(false));
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
fn a_route_that_takes_the_foreground_is_allowed_exactly_where_the_chain_runs_it() {
    assert!(!foreground_route_allowed(InteractionPolicy::headless()));
    assert!(foreground_route_allowed(InteractionPolicy::focus_fallback()));
    assert!(foreground_route_allowed(InteractionPolicy::headed()));
    for policy in [
        InteractionPolicy::headless(),
        InteractionPolicy::focus_fallback(),
        InteractionPolicy::headed(),
    ] {
        let calls = Cell::new((0, 0));
        let result = run_two(
            policy,
            (false, DeliveryOutcome::NotDelivered),
            (
                true,
                DeliveryOutcome::Delivered(DeliveryObservation::Verified),
            ),
            &calls,
        );
        assert_eq!(result.is_ok(), foreground_route_allowed(policy));
        assert_eq!(calls.get().1 == 1, foreground_route_allowed(policy));
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

#[test]
fn a_press_whose_target_vanishes_reports_delivered_unverified_and_no_safe_retry() {
    use crate::actions::mutation::classify_write_failure;
    use crate::system::runtime::hresult::UIA_E_ELEMENTNOTAVAILABLE;
    use crate::tree::automation::UiaFailure;
    use agent_desktop_core::{Action, ActionResult, DeliveryDisposition, RetryDisposition};

    let mut press = || {
        let delivered = classify_write_failure(
            "DoDefaultAction",
            LEGACY,
            UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE),
        )?;
        Ok(DeliveryOutcome::from_delivery(delivered, false))
    };
    let steps = execute_chain(
        &mut offered([rung(LEGACY, true, false, &mut press)]),
        InteractionPolicy::headless(),
        deadline(5_000),
        "retry",
    )
    .expect("a vanished target is not an error");
    let result = ActionResult::from_execution(&Action::Click, steps);

    assert_eq!(
        result.disposition().delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    assert_eq!(result.disposition().retry(), RetryDisposition::Unsafe);
}

#[test]
fn a_pattern_that_cannot_be_obtained_stays_a_stale_reference_with_a_safe_retry() {
    use crate::actions::mutation::classify_write_failure;
    use crate::system::runtime::hresult::UIA_E_ELEMENTNOTAVAILABLE;
    use crate::tree::automation::UiaFailure;
    use agent_desktop_core::RetryDisposition;

    let error = classify_write_failure(
        "get_pattern",
        LEGACY,
        UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE),
    )
    .expect_err("nothing was sent");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
}

#[test]
fn an_expired_deadline_still_reads_once_instead_of_timing_out() {
    let cap = Duration::from_millis(600);
    let reads = Cell::new(0);
    let reached = poll_until(deadline(0), cap, || {
        reads.set(reads.get() + 1);
        true
    });
    assert!(reached);
    assert_eq!(reads.get(), 1);

    reads.set(0);
    let reached = poll_until(deadline(0), cap, || {
        reads.set(reads.get() + 1);
        false
    });
    assert!(!reached);
    assert_eq!(reads.get(), 1);
}
