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
/// A rung whose effects `policy` does not permit is held back and records no
/// step. A rung that sends nothing hands over to the next one. A rung that
/// finds the element already in state, or that sends the action and sees its
/// effect or cannot rule one out, ends the chain; only a settled read-back of
/// no effect lets the next rung try, so no rung acts twice. An error stops
/// the chain, and one raised after an earlier rung delivered keeps that
/// delivery in its disposition. When nothing was delivered and a rung was
/// held back, the chain is `POLICY_DENIED` naming the held-back mechanisms,
/// so the caller learns that `--headed` would act; otherwise an exhausted
/// chain is `ACTION_FAILED` with `suggestion` and the rungs it tried.
pub fn execute_chain(
    rungs: &mut [ChainRung<'_>],
    policy: InteractionPolicy,
    deadline: Deadline,
    suggestion: &'static str,
) -> Result<Vec<ActionStep>, AdapterError> {
    let mut steps = Vec::new();
    let mut withheld = Vec::new();
    for rung in rungs.iter_mut() {
        if let Err(expired) = deadline.ensure_remaining() {
            return Err(expired.with_disposition(settled_disposition(&steps)));
        }
        if !permitted(rung, policy) {
            withheld.push(rung.label);
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
    if !withheld.is_empty() && !delivered(&steps) {
        return Err(mechanisms_denied(&withheld, policy));
    }
    Err(exhausted(&steps, suggestion))
}

fn permitted(rung: &ChainRung<'_>, policy: InteractionPolicy) -> bool {
    rung.effects
        .iter()
        .all(|effect| policy.require(*effect, rung.label).is_ok())
}

/// The refusal for an action whose only mechanisms left, named by `labels`,
/// take the foreground the interaction policy does not allow. Nothing was sent.
pub fn mechanisms_denied(labels: &[&'static str], policy: InteractionPolicy) -> AdapterError {
    AdapterError::policy_denied_for_policy(
        format!(
            "{} takes the foreground, which the interaction policy does not allow",
            labels.join(" and ")
        ),
        policy,
    )
    .with_details(json!({ "foreground_effect": true, "mechanisms": labels }))
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
