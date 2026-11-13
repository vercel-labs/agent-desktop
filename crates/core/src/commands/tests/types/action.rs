use crate::*;
use serde_json::json;

#[rustfmt::skip]
fn full_result() -> ActionResult {
    let mut result = ActionResult::delivered_unverified("set-value")
        .with_state(ElementState { role: "checkbox".into(), states: vec!["checked".into()], value: Some("1".into()), enabled: Some(true), hidden: Some(false), offscreen: Some(false) })
        .with_steps(vec![ActionStep::attempted("focus"), ActionStep::succeeded("write").with_mechanism(StepMechanism::SemanticApi).with_verified(true)])
        .with_details(json!({"verification_scope": "element_value"}))
        .with_verified_delivery();
    result.surfaces = vec![
        SurfaceInfo { id: "menu-1".into(), kind: "menu".into(), title: Some("File".into()), item_count: Some(12), unclassified: vec!["sheet".into()] },
        SurfaceInfo { id: "alert-1".into(), kind: "alert".into(), title: None, item_count: None, unclassified: Vec::new() },
    ];
    result
}

#[test]
fn element_state_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-element-state.json"),
        json!({
            "minimal": ElementState { role: "button".into(), states: Vec::new(), value: None, enabled: None, hidden: None, offscreen: None },
            "observed_false_flags": ElementState { role: "button".into(), states: Vec::new(), value: Some(String::new()), enabled: Some(false), hidden: Some(true), offscreen: Some(true) },
        })
    );
}

#[test]
fn action_step_variants() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-action-step.json"),
        json!([
            ActionStep::attempted("focus"),
            ActionStep::skipped("scroll"),
            ActionStep::succeeded("press"),
            ActionStep::succeeded("press")
                .with_mechanism(StepMechanism::PhysicalSynthetic)
                .with_verified(false),
            ActionStep::skipped("press")
                .with_mechanism(StepMechanism::SemanticApi)
                .with_verified(true),
        ])
    );
}

#[test]
fn action_result_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-action-result.json"),
        json!({
            "full": full_result(),
            "delivered_unverified": ActionResult::delivered_unverified("click"),
            "satisfied": ActionResult::satisfied_without_delivery("toggle"),
        })
    );
}

#[test]
fn delivery_semantics_values() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-delivery-semantics.json"),
        json!({
            "unknown": DeliverySemantics::unknown(),
            "not_delivered": DeliverySemantics::not_delivered(),
            "delivery_uncertain": DeliverySemantics::uncertain(),
            "delivered_unverified": DeliverySemantics::delivered_unverified(),
            "delivered_verified": DeliverySemantics::delivered_verified(),
        })
    );
}

#[test]
fn operation_report_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-operation-report.json"),
        json!({
            "full": OperationReport::new(full_result(), OperationTarget::Window { id: "w-12".into(), title: "Untitled".into(), process: ProcessIdentity::new(4242_u32, "start-time:1700000000") }),
            "minimal": OperationReport::satisfied_without_delivery("clipboard-clear", OperationTarget::Clipboard),
        })
    );
}
