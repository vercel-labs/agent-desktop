use super::settle;
use crate::{ActionResult, AdapterError, DeliverySemantics, ErrorCode, KeyCombo, Modifier};

fn key(name: &str, modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: name.into(),
        modifiers,
    }
}

fn refusal(check: &str) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::stale_ref_because("not focused")
        .with_details(serde_json::json!({ "check": check }))
        .with_disposition(DeliverySemantics::not_delivered()))
}

fn closed() -> Result<Option<ActionResult>, AdapterError> {
    Ok(Some(
        ActionResult::delivered_unverified("collapse").with_verified_delivery(),
    ))
}

#[test]
fn a_focus_refusal_of_a_plain_escape_closes_the_open_surface() {
    for check in ["window_active", "exact_window_focus", "process_focus"] {
        let result = settle(refusal(check), &key("escape", vec![]), closed).unwrap();
        assert_eq!(result.action, "press");
        assert_eq!(
            result.disposition(),
            DeliverySemantics::delivered_verified()
        );
    }
}

#[test]
fn a_close_attempt_that_failed_reports_its_own_error() {
    let attempt = || -> Result<Option<ActionResult>, AdapterError> {
        Err(AdapterError::new(ErrorCode::Timeout, "no reply")
            .with_disposition(DeliverySemantics::uncertain()))
    };
    let error = settle(refusal("window_active"), &key("escape", vec![]), attempt).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
}

#[test]
fn the_refusal_stands_without_an_open_surface_or_a_close_route() {
    for close in [
        || Ok(None),
        || Err(AdapterError::not_supported("close_transient_surface")),
    ] as [fn() -> Result<Option<ActionResult>, AdapterError>; 2]
    {
        let error = settle(refusal("window_active"), &key("escape", vec![]), close).unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleRef);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}

#[test]
fn every_other_key_result_is_left_as_the_key_route_gave_it() {
    let never = || -> Result<Option<ActionResult>, AdapterError> { panic!("no close") };
    let delivered = Ok(ActionResult::delivered_unverified("press"));
    assert!(settle(delivered, &key("escape", vec![]), never).is_ok());
    assert!(
        settle(
            refusal("window_active"),
            &key("escape", vec![Modifier::Shift]),
            never
        )
        .is_err()
    );
    assert!(settle(refusal("window_active"), &key("return", vec![]), never).is_err());
    assert!(settle(refusal("geometry"), &key("escape", vec![]), never).is_err());
    let sent = Err(AdapterError::new(ErrorCode::ActionFailed, "sent")
        .with_details(serde_json::json!({ "check": "window_active" }))
        .with_disposition(DeliverySemantics::delivered_unverified()));
    assert!(settle(sent, &key("escape", vec![]), never).is_err());
}
