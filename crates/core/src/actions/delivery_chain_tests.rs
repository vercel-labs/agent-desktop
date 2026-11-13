use super::*;
use crate::wait::deadline::VirtualClock;
use std::cell::Cell;
use std::time::Duration;

const SUGGESTION: &str = "Refresh the snapshot and retry.";

fn run(
    outcomes: &[Result<DeliveryOutcome, AdapterError>],
    effects: &[&'static [Effect]],
    policy: InteractionPolicy,
) -> (Result<Vec<ActionStep>, AdapterError>, Vec<u32>) {
    let calls: Vec<Cell<u32>> = outcomes.iter().map(|_| Cell::new(0)).collect();
    let mut closures: Vec<_> = outcomes
        .iter()
        .zip(&calls)
        .map(|(outcome, calls)| {
            move || {
                calls.set(calls.get() + 1);
                outcome.clone()
            }
        })
        .collect();
    let labels = ["first", "second", "third"];
    let mut rungs: Vec<ChainRung<'_>> = closures
        .iter_mut()
        .enumerate()
        .map(|(index, run)| ChainRung {
            label: labels[index],
            effects: effects.get(index).copied().unwrap_or(&[]),
            run: Box::new(run),
        })
        .collect();
    let result = execute_chain(
        &mut rungs,
        policy,
        Deadline::after(5_000).unwrap(),
        SUGGESTION,
    );
    (result, calls.iter().map(Cell::get).collect())
}

fn delivered(observation: DeliveryObservation) -> Result<DeliveryOutcome, AdapterError> {
    Ok(DeliveryOutcome::Delivered(observation))
}

#[test]
fn the_chain_stops_at_the_first_rung_that_settles_the_action() {
    let (steps, calls) = run(
        &[
            Ok(DeliveryOutcome::NotDelivered),
            delivered(DeliveryObservation::Verified),
            delivered(DeliveryObservation::Verified),
        ],
        &[],
        InteractionPolicy::headless(),
    );
    let steps = steps.unwrap();
    assert_eq!(calls, [1, 1, 0]);
    assert_eq!(steps.len(), 2);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Skipped));
    assert_eq!(steps[1].verified(), Some(true));
    assert_eq!(steps[1].mechanism(), Some(StepMechanism::SemanticApi));
}

#[test]
fn only_a_settled_no_effect_lets_the_next_rung_act() {
    for (observation, next_runs, verified) in [
        (DeliveryObservation::NoEffect, 1, Some(false)),
        (DeliveryObservation::Unverified, 0, Some(false)),
        (DeliveryObservation::Unread, 0, None),
    ] {
        let (steps, calls) = run(
            &[
                delivered(observation),
                delivered(DeliveryObservation::Verified),
            ],
            &[],
            InteractionPolicy::headless(),
        );
        assert_eq!(calls[1], next_runs, "{observation:?}");
        let steps = steps.unwrap();
        assert!(matches!(steps[0].outcome, ActionStepOutcome::Succeeded));
        assert_eq!(steps[0].verified(), verified, "{observation:?}");
    }
}

#[test]
fn an_element_already_in_state_ends_the_chain_without_delivery() {
    let (steps, calls) = run(
        &[
            Ok(DeliveryOutcome::Satisfied),
            delivered(DeliveryObservation::Verified),
        ],
        &[],
        InteractionPolicy::headless(),
    );
    let steps = steps.unwrap();
    assert_eq!(calls, [1, 0]);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Skipped));
    assert_eq!(steps[0].verified(), Some(true));
}

#[test]
fn an_error_after_a_delivery_keeps_that_delivery_in_its_disposition() {
    let failure = || {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "provider failed",
        ))
    };
    let (before, _) = run(&[failure()], &[], InteractionPolicy::headless());
    assert_eq!(before.unwrap_err().disposition, DeliverySemantics::Unknown);
    let (after, _) = run(
        &[delivered(DeliveryObservation::NoEffect), failure()],
        &[],
        InteractionPolicy::headless(),
    );
    assert_eq!(
        after.unwrap_err().disposition,
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn a_rung_the_policy_denies_is_skipped_and_reported_only_when_nothing_ran() {
    let physical: &'static [Effect] = &[Effect::FocusSteal, Effect::CursorMove];
    let (steps, calls) = run(
        &[
            delivered(DeliveryObservation::Verified),
            delivered(DeliveryObservation::Verified),
        ],
        &[physical, &[]],
        InteractionPolicy::headless(),
    );
    assert_eq!(calls, [0, 1]);
    assert_eq!(steps.unwrap().len(), 1);

    let (denied, calls) = run(
        &[delivered(DeliveryObservation::Verified)],
        &[physical],
        InteractionPolicy::headless(),
    );
    assert_eq!(calls, [0]);
    assert_eq!(denied.unwrap_err().code, ErrorCode::PolicyDenied);

    let (headed, _) = run(
        &[delivered(DeliveryObservation::Verified)],
        &[physical],
        InteractionPolicy::headed(),
    );
    assert_eq!(
        headed.unwrap()[0].mechanism(),
        Some(StepMechanism::PhysicalSynthetic)
    );
}

#[test]
fn an_exhausted_chain_names_the_rungs_it_tried() {
    let (result, _) = run(
        &[
            Ok(DeliveryOutcome::NotDelivered),
            Ok(DeliveryOutcome::NotDelivered),
        ],
        &[],
        InteractionPolicy::headless(),
    );
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.suggestion.as_deref(), Some(SUGGESTION));
    assert_eq!(
        error.details.unwrap()["steps_tried"],
        json!(["first", "second"])
    );
}

#[test]
fn an_expired_budget_stops_before_the_next_rung_and_keeps_any_delivery() {
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(100).unwrap();
    let mut first = || {
        clock.advance(Duration::from_millis(200));
        Ok(DeliveryOutcome::Delivered(DeliveryObservation::NoEffect))
    };
    let mut second = || -> Result<DeliveryOutcome, AdapterError> { panic!("the budget is spent") };
    let error = execute_chain(
        &mut [
            ChainRung {
                label: "first",
                effects: &[],
                run: Box::new(&mut first),
            },
            ChainRung {
                label: "second",
                effects: &[],
                run: Box::new(&mut second),
            },
        ],
        InteractionPolicy::headless(),
        deadline,
        SUGGESTION,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_rung_that_sent_nothing_is_not_delivered_whatever_it_read() {
    assert_eq!(
        DeliveryOutcome::from_delivery(false, true),
        DeliveryOutcome::NotDelivered
    );
    assert_eq!(
        DeliveryOutcome::from_delivery(true, false),
        DeliveryOutcome::Delivered(DeliveryObservation::Unverified)
    );
}

#[test]
fn exhaustion_and_early_errors_keep_the_truth_about_delivery() {
    let (after_no_effect, _) = run(
        &[
            delivered(DeliveryObservation::NoEffect),
            Ok(DeliveryOutcome::NotDelivered),
        ],
        &[],
        InteractionPolicy::headless(),
    );
    assert_eq!(
        after_no_effect.unwrap_err().disposition,
        DeliverySemantics::delivered_unverified()
    );
    let uncertain = AdapterError::new(ErrorCode::ActionFailed, "transport lost")
        .with_disposition(DeliverySemantics::uncertain());
    let (early, _) = run(&[Err(uncertain)], &[], InteractionPolicy::headless());
    assert_eq!(
        early.unwrap_err().disposition,
        DeliverySemantics::uncertain()
    );
}
