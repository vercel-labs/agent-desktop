use super::*;
use crate::ErrorCode;

#[test]
fn a_headless_policy_refuses_to_unminimize_with_nothing_sent_and_names_headed() {
    let error = WindowOp::require_unminimize_allowed(InteractionPolicy::headless()).unwrap_err();
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert!(error.suggestion.unwrap().contains("--headed"));
    assert!(error.message.contains("restore of a minimized window"));
}

#[test]
fn a_policy_that_lets_a_window_take_the_focus_may_unminimize() {
    assert!(WindowOp::require_unminimize_allowed(InteractionPolicy::headed()).is_ok());
    assert!(WindowOp::require_unminimize_allowed(InteractionPolicy::focus_fallback()).is_ok());
}
