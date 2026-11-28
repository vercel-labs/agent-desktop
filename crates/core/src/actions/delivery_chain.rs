//! The one engine that runs an action's mechanisms in order until one of them
//! settles the action.

use serde_json::json;

use crate::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    actions::{chain_rung::ChainRung, delivery_outcome::DeliveryOutcome},
};

/// Runs `rungs` in order and returns the steps it recorded.
///
/// A rung whose effects `policy` does not permit is held back and records no
/// step. A rung that sends nothing hands over to the next one. A rung that
/// finds the element already in state, or that sends the action, ends the
/// chain whatever it read back: an acknowledged send whose effect is not seen
/// yet may still land, so no later rung may act on top of it. An error stops
/// the chain. When a rung was held back, the chain is `POLICY_DENIED` naming
/// the held-back mechanisms, so the caller learns that `--headed` would act;
/// otherwise an exhausted chain is `ACTION_FAILED` with `suggestion` and the
/// rungs it tried.
pub fn execute_chain(
    rungs: &mut [ChainRung<'_>],
    policy: InteractionPolicy,
    deadline: Deadline,
    suggestion: &'static str,
) -> Result<Vec<ActionStep>, AdapterError> {
    let mut steps = Vec::new();
    let mut withheld = Vec::new();
    for rung in rungs.iter_mut() {
        deadline
            .ensure_remaining()
            .map_err(|expired| expired.with_disposition(DeliverySemantics::not_delivered()))?;
        if !permitted(rung, policy) {
            withheld.push(rung.label);
            continue;
        }
        let outcome = (rung.run)()?;
        steps.push(outcome.step(rung.label).with_mechanism(rung.mechanism));
        if outcome != DeliveryOutcome::NotDelivered {
            return Ok(steps);
        }
    }
    if !withheld.is_empty() {
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

fn exhausted(steps: &[ActionStep], suggestion: &'static str) -> AdapterError {
    let rungs: Vec<_> = steps
        .iter()
        .map(|step| {
            json!({ "label": step.label(), "outcome": "not_delivered", "cause": "affordance_absent" })
        })
        .collect();
    AdapterError::new(
        ErrorCode::ActionFailed,
        "No step delivered the action: every step answered false or named an affordance the element does not carry",
    )
    .with_suggestion(suggestion)
    .with_details(json!({
        "steps_tried": steps.iter().map(ActionStep::label).collect::<Vec<_>>(),
        "rungs": rungs,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "delivery_chain_tests.rs"]
mod tests;
