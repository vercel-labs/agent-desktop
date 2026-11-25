use super::*;

#[test]
fn a_refusal_without_a_disposition_is_not_delivered_and_safe() {
    for action in [Action::Click, Action::Select("x".into())] {
        for code in [ErrorCode::ActionNotSupported, ErrorCode::InvalidArgs] {
            let mut adapter = adapter(element(None, &[]), element(None, &[]));
            adapter.execution = Err(AdapterError::new(code, "refused"));
            let error = execute(&adapter, action.clone()).unwrap_err();
            assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        }
    }
}

#[test]
fn a_refusal_that_reports_a_send_keeps_its_disposition() {
    let mut adapter = adapter(element(None, &[]), element(None, &[]));
    adapter.execution = Err(AdapterError::new(ErrorCode::ActionNotSupported, "late")
        .with_disposition(DeliverySemantics::delivered_unverified()));
    let error = execute(&adapter, Action::Click).unwrap_err();
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn an_error_of_another_kind_without_a_disposition_is_never_marked_not_delivered() {
    let mut adapter = adapter(element(None, &[]), element(None, &[]));
    adapter.execution = Err(AdapterError::timeout("late"));
    let error = execute(&adapter, Action::Click).unwrap_err();
    assert_ne!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn an_action_failed_with_an_uncertain_disposition_stays_uncertain() {
    let mut adapter = adapter(element(None, &[]), element(None, &[]));
    adapter.execution = Err(AdapterError::new(ErrorCode::ActionFailed, "after dispatch")
        .with_disposition(DeliverySemantics::uncertain()));
    let error = execute(&adapter, Action::Click).unwrap_err();
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
}
