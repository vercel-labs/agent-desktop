use std::collections::VecDeque;
use std::sync::Mutex;

use super::*;
use crate::{Action, ActionOps, Deadline, InputOps, ObservationOps, ObservedEffect, SystemOps};

/// Witness reads in order: the first is the read before the action.
struct Adapter {
    reads: Mutex<VecDeque<Result<EffectWitness, AdapterError>>>,
    dispatch: Result<ActionResult, AdapterError>,
    read_count: Mutex<usize>,
}

impl ObservationOps for Adapter {
    fn read_effect_witness(
        &self,
        _: &NativeHandle,
        _: Deadline,
    ) -> Result<EffectWitness, AdapterError> {
        *self.read_count.lock().unwrap() += 1;
        let mut reads = self.reads.lock().unwrap();
        if reads.len() > 1 {
            reads.pop_front().unwrap()
        } else {
            reads.front().cloned().unwrap()
        }
    }
}

impl ActionOps for Adapter {
    fn execute_action(
        &self,
        _: &NativeHandle,
        _: ActionRequest,
        _: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatch.clone()
    }
}

impl InputOps for Adapter {}
impl SystemOps for Adapter {}

fn witness(selected: bool, title: &str) -> EffectWitness {
    let mut witness = EffectWitness::default();
    witness.target.selected = Some(selected);
    witness.window.title = Some(title.into());
    witness
}

fn adapter(
    reads: Vec<Result<EffectWitness, AdapterError>>,
    dispatch: Result<ActionResult, AdapterError>,
) -> Adapter {
    Adapter {
        reads: Mutex::new(reads.into()),
        dispatch,
        read_count: Mutex::new(0),
    }
}

fn run(adapter: &Adapter) -> Result<ActionResult, AdapterError> {
    execute_witnessed_action(
        adapter,
        &NativeHandle::null(),
        ActionRequest::headless(Action::Click),
        &InteractionLease::guarded(Deadline::after(2000).unwrap(), ()).unwrap(),
    )
}

fn unverified() -> Result<ActionResult, AdapterError> {
    Ok(ActionResult::delivered_unverified("click"))
}

fn uncertain() -> Result<ActionResult, AdapterError> {
    Err(AdapterError::new(ErrorCode::ActionFailed, "unclear")
        .with_disposition(DeliverySemantics::uncertain()))
}

#[test]
fn the_dispatch_result_chooses_the_step() {
    assert_eq!(witness_step(&unverified()), WitnessStep::Read);
    assert_eq!(witness_step(&uncertain()), WitnessStep::Read);
    let verified = ActionResult::delivered_unverified("click").with_verified_delivery();
    assert_eq!(witness_step(&Ok(verified)), WitnessStep::Leave);
    let refused = Err(AdapterError::new(ErrorCode::PolicyDenied, "no")
        .with_disposition(DeliverySemantics::not_delivered()));
    assert_eq!(witness_step(&refused), WitnessStep::Leave);
}

#[test]
fn a_strong_change_verifies_a_delivery_with_one_read() {
    let adapter = adapter(
        vec![Ok(witness(false, "A")), Ok(witness(true, "A"))],
        unverified(),
    );
    let result = run(&adapter).unwrap();

    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(result.effect, Some(ObservedEffect::Observed));
    assert_eq!(
        result.details.unwrap()["witness_diff"],
        serde_json::json!(["target_selected"])
    );
    assert_eq!(*adapter.read_count.lock().unwrap(), 2);
}

#[test]
fn a_weak_or_no_change_leaves_the_delivery_as_it_was() {
    for (after, effect) in [
        (witness(false, "B"), ObservedEffect::Indeterminate),
        (witness(false, "A"), ObservedEffect::NotObserved),
    ] {
        let adapter = adapter(vec![Ok(witness(false, "A")), Ok(after)], unverified());
        let result = run(&adapter).unwrap();

        assert_eq!(
            result.disposition(),
            DeliverySemantics::delivered_unverified()
        );
        assert_eq!(result.effect, Some(effect));
    }
}

#[test]
fn an_unclear_answer_with_a_strong_change_is_a_verified_success() {
    let adapter = adapter(
        vec![Ok(witness(false, "A")), Ok(witness(true, "A"))],
        uncertain(),
    );
    let result = run(&adapter).expect("an observed effect is a success");

    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(result.effect, Some(ObservedEffect::Observed));
    let details = result.details.unwrap();
    assert_eq!(
        details["witness_diff"],
        serde_json::json!(["target_selected"])
    );
    assert_eq!(details["platform_report"]["message"], "unclear");
    assert_eq!(*adapter.read_count.lock().unwrap(), 2);
}

#[test]
fn an_unclear_answer_with_no_change_stays_uncertain_without_a_wait() {
    let adapter = adapter(
        vec![
            Ok(witness(false, "A")),
            Ok(witness(false, "A")),
            Ok(witness(true, "A")),
        ],
        uncertain(),
    );
    let started = std::time::Instant::now();
    let error = run(&adapter).unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::uncertain());
    assert_eq!(error.details.unwrap()["effect"], "not_observed");
    assert_eq!(*adapter.read_count.lock().unwrap(), 2);
    assert!(started.elapsed() < std::time::Duration::from_millis(40));
}

#[test]
fn a_target_gone_after_the_action_is_a_strong_change() {
    let adapter = adapter(
        vec![
            Ok(witness(false, "A")),
            Err(AdapterError::stale_ref_because("gone")),
        ],
        unverified(),
    );
    let result = run(&adapter).unwrap();

    assert_eq!(result.effect, Some(ObservedEffect::Observed));
    assert_eq!(
        result.details.unwrap()["witness_diff"],
        serde_json::json!(["target_gone"])
    );
}

#[test]
fn no_witness_and_an_undelivered_action_are_left_alone() {
    let unsupported = adapter(
        vec![Err(AdapterError::not_supported("read_effect_witness"))],
        unverified(),
    );
    let result = run(&unsupported).unwrap();
    assert_eq!(result.effect, None);
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );

    let refused = adapter(
        vec![Ok(witness(false, "A"))],
        Err(AdapterError::new(ErrorCode::PolicyDenied, "no")
            .with_disposition(DeliverySemantics::not_delivered())),
    );
    assert_eq!(
        run(&refused).unwrap_err().disposition,
        DeliverySemantics::not_delivered()
    );
    assert_eq!(*refused.read_count.lock().unwrap(), 1);
}

#[test]
fn a_delivery_without_proof_is_read_once_without_a_wait() {
    let adapter = adapter(
        vec![
            Ok(witness(false, "A")),
            Ok(witness(false, "A")),
            Ok(witness(true, "A")),
        ],
        unverified(),
    );
    let result = run(&adapter).unwrap();

    assert_eq!(result.effect, Some(ObservedEffect::NotObserved));
    assert_eq!(*adapter.read_count.lock().unwrap(), 2);
}
