use super::*;
use crate::wait::deadline::VirtualClock;
use crate::{
    ActionStepOutcome, Effect, StepMechanism, actions::delivery_observation::DeliveryObservation,
};
use std::cell::Cell;
use std::time::Duration;

const SUGGESTION: &str = "Refresh the snapshot and retry.";

type RungShape = (&'static [Effect], StepMechanism);

const SEMANTIC: RungShape = (&[], StepMechanism::SemanticApi);

const PHYSICAL: RungShape = (
    &[Effect::FocusSteal, Effect::CursorMove],
    StepMechanism::PhysicalSynthetic,
);

fn run(
    outcomes: &[Result<DeliveryOutcome, AdapterError>],
    shapes: &[RungShape],
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
        .map(|(index, run)| {
            let (effects, mechanism) = shapes.get(index).copied().unwrap_or(SEMANTIC);
            ChainRung {
                label: labels[index],
                effects,
                mechanism,
                run: Box::new(run),
            }
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
fn a_delivered_rung_ends_the_chain_whatever_it_read_back() {
    for (observation, verified) in [
        (DeliveryObservation::Verified, Some(true)),
        (DeliveryObservation::Unverified, Some(false)),
        (DeliveryObservation::Unread, None),
    ] {
        let (steps, calls) = run(
            &[
                delivered(observation),
                delivered(DeliveryObservation::Verified),
            ],
            &[],
            InteractionPolicy::headless(),
        );
        assert_eq!(calls[1], 0, "{observation:?}");
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
fn a_rung_error_keeps_its_own_disposition() {
    let failure = || {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "provider failed",
        ))
    };
    let (error, _) = run(&[failure()], &[], InteractionPolicy::headless());
    assert_eq!(error.unwrap_err().disposition, DeliverySemantics::Unknown);
}

#[test]
fn a_rung_the_policy_denies_is_held_back_and_named_when_nothing_was_delivered() {
    let (steps, calls) = run(
        &[
            delivered(DeliveryObservation::Verified),
            delivered(DeliveryObservation::Verified),
        ],
        &[PHYSICAL, SEMANTIC],
        InteractionPolicy::headless(),
    );
    assert_eq!(calls, [0, 1]);
    assert_eq!(steps.unwrap().len(), 1);

    let (denied, calls) = run(
        &[
            Ok(DeliveryOutcome::NotDelivered),
            delivered(DeliveryObservation::Verified),
        ],
        &[SEMANTIC, PHYSICAL],
        InteractionPolicy::headless(),
    );
    assert_eq!(calls, [1, 0]);
    let denied = denied.unwrap_err();
    assert_eq!(denied.code, ErrorCode::PolicyDenied);
    assert_eq!(denied.disposition, DeliverySemantics::not_delivered());
    let details = denied.details.unwrap();
    assert_eq!(details["foreground_effect"], true);
    assert_eq!(details["mechanisms"], serde_json::json!(["second"]));

    let (headed, _) = run(
        &[delivered(DeliveryObservation::Verified)],
        &[PHYSICAL],
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
fn an_expired_budget_stops_before_the_next_rung() {
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(100).unwrap();
    let mut first = || {
        clock.advance(Duration::from_millis(200));
        Ok(DeliveryOutcome::NotDelivered)
    };
    let mut second = || -> Result<DeliveryOutcome, AdapterError> { panic!("the budget is spent") };
    let error = execute_chain(
        &mut [
            ChainRung {
                label: "first",
                effects: &[],
                mechanism: StepMechanism::SemanticApi,
                run: Box::new(&mut first),
            },
            ChainRung {
                label: "second",
                effects: &[],
                mechanism: StepMechanism::SemanticApi,
                run: Box::new(&mut second),
            },
        ],
        InteractionPolicy::headless(),
        deadline,
        SUGGESTION,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
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
fn an_early_uncertain_error_keeps_its_disposition() {
    let uncertain = AdapterError::new(ErrorCode::ActionFailed, "transport lost")
        .with_disposition(DeliverySemantics::uncertain());
    let (early, _) = run(&[Err(uncertain)], &[], InteractionPolicy::headless());
    assert_eq!(
        early.unwrap_err().disposition,
        DeliverySemantics::uncertain()
    );
}

#[test]
fn a_refusal_annotated_by_an_outer_layer_keeps_its_mechanisms() {
    let error = mechanisms_denied(&["ValuePattern.SetValue"], InteractionPolicy::headless())
        .with_details(json!({ "value_chars": 3 }));
    let details = error.details.unwrap();
    assert_eq!(details["foreground_effect"], json!(true));
    assert_eq!(details["mechanisms"], json!(["ValuePattern.SetValue"]));
    assert_eq!(details["value_chars"], json!(3));
}
