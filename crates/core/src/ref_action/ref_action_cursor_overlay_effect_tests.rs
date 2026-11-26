use super::*;

#[test]
fn physical_effect_uses_verified_input_point_and_requires_delivery() {
    let adapter = CursorAdapter::new(false);
    let context = enabled_context().with_headed(true);
    let entry = entry();
    let target = crate::ref_action::ref_action_wait_context::RefActionWaitContext {
        adapter: &adapter,
        entry: &entry,
        ref_id: "@e1",
        context: &context,
    };
    let preflight = ActionabilityPreflight {
        verified_point: Some(crate::Point { x: 4.0, y: 5.0 }),
        presentation_point: Some(crate::Point { x: 11.0, y: 11.0 }),
        presentation_bounds: entry.geometry.bounds,
        pointer_delivery: actionability::PointerDelivery::Physical,
    };
    presentation::after_dispatch(
        &target,
        &preflight,
        true,
        &Err(AdapterError::stale_ref("blocked")
            .with_disposition(crate::DeliverySemantics::NotDelivered)),
    );
    assert!(adapter.presented.lock().unwrap().is_empty());
    presentation::after_dispatch(
        &target,
        &preflight,
        true,
        &Ok(ActionResult::delivered_unverified("click")),
    );
    let presented = adapter.presented.lock().unwrap();
    assert_eq!(presented.len(), 1);
    assert_eq!(
        presented[0].instruction().unwrap().destination(),
        &crate::Point { x: 4.0, y: 5.0 }
    );
}
