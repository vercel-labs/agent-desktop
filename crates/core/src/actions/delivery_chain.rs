//! The one engine that runs an action's mechanisms in order until one of them
//! settles the action.

use serde_json::json;

use crate::{
    ActionStep, ActionStepOutcome, AdapterError, Deadline, DeliverySemantics, Effect, ErrorCode,
    InteractionPolicy, StepMechanism,
    actions::{
        chain_rung::ChainRung, delivery_observation::DeliveryObservation,
        delivery_outcome::DeliveryOutcome,
    },
};

/// Runs `rungs` in order and returns the steps it recorded.
///
/// A rung whose effects `policy` does not permit is skipped and records no
/// step. A rung that sends nothing hands over to the next one. A rung that
/// finds the element already in state, or that sends the action and sees its
/// effect or cannot rule one out, ends the chain; only a settled read-back of
/// no effect lets the next rung try, so no rung acts twice. An error stops
/// the chain, and one raised after an earlier rung delivered keeps that
/// delivery in its disposition. When every rung is skipped by the policy the
/// refusal is returned; otherwise an exhausted chain is `ACTION_FAILED` with
/// `suggestion` and the rungs it tried.
pub fn execute_chain(
    rungs: &mut [ChainRung<'_>],
    policy: InteractionPolicy,
    deadline: Deadline,
    suggestion: &'static str,
) -> Result<Vec<ActionStep>, AdapterError> {
    let mut steps = Vec::new();
    let mut refusal = None;
    for rung in rungs.iter_mut() {
        if let Err(expired) = deadline.ensure_remaining() {
            return Err(expired.with_disposition(settled_disposition(&steps)));
        }
        if let Err(denied) = permitted(rung, policy) {
            refusal.get_or_insert(denied);
            continue;
        }
        let outcome = match (rung.run)() {
            Ok(outcome) => outcome,
            Err(error) if delivered(&steps) => {
                return Err(error.with_disposition(DeliverySemantics::delivered_unverified()));
            }
            Err(error) => return Err(error),
        };
        steps.push(
            outcome
                .step(rung.label)
                .with_mechanism(mechanism_of(rung.effects)),
        );
        match outcome {
            DeliveryOutcome::NotDelivered
            | DeliveryOutcome::Delivered(DeliveryObservation::NoEffect) => {}
            DeliveryOutcome::Satisfied | DeliveryOutcome::Delivered(_) => return Ok(steps),
        }
    }
    match refusal {
        Some(refusal) if steps.is_empty() => Err(refusal),
        _ => Err(exhausted(&steps, suggestion)),
    }
}

fn permitted(rung: &ChainRung<'_>, policy: InteractionPolicy) -> Result<(), AdapterError> {
    rung.effects
        .iter()
        .try_for_each(|effect| policy.require(*effect, rung.label))
}

fn mechanism_of(effects: &[Effect]) -> StepMechanism {
    if effects.contains(&Effect::CursorMove) {
        StepMechanism::PhysicalSynthetic
    } else {
        StepMechanism::SemanticApi
    }
}

fn delivered(steps: &[ActionStep]) -> bool {
    steps
        .iter()
        .any(|step| matches!(step.outcome, ActionStepOutcome::Succeeded))
}

fn settled_disposition(steps: &[ActionStep]) -> DeliverySemantics {
    if delivered(steps) {
        DeliverySemantics::delivered_unverified()
    } else {
        DeliverySemantics::not_delivered()
    }
}

fn exhausted(steps: &[ActionStep], suggestion: &'static str) -> AdapterError {
    let rungs: Vec<_> = steps
        .iter()
        .map(|step| match step.outcome {
            ActionStepOutcome::Succeeded => {
                json!({ "label": step.label(), "outcome": "delivered", "observed": "no_effect" })
            }
            _ => {
                json!({ "label": step.label(), "outcome": "not_delivered", "cause": "affordance_absent" })
            }
        })
        .collect();
    let message = if delivered(steps) {
        "Every step that delivered the action observed no effect"
    } else {
        "No step delivered the action: every step answered false or named an affordance the element does not carry"
    };
    AdapterError::new(ErrorCode::ActionFailed, message)
        .with_suggestion(suggestion)
        .with_details(json!({
            "steps_tried": steps.iter().map(ActionStep::label).collect::<Vec<_>>(),
            "rungs": rungs,
        }))
        .with_disposition(settled_disposition(steps))
}

#[cfg(test)]
#[path = "delivery_chain_tests.rs"]
mod tests;
