use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{Action, Rect};

struct NoPointerSession;
impl ObservationOps for NoPointerSession {}
impl ActionOps for NoPointerSession {}
impl SystemOps for NoPointerSession {}
impl InputOps for NoPointerSession {
    fn physical_pointer_refusal(&self) -> Option<AdapterError> {
        Some(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "no pointer",
        ))
    }
}

fn evidence(role: &str, actions: &[&str]) -> ActionabilityEvidence {
    ActionabilityEvidence {
        nonspatial: false,
        state: crate::ElementState {
            role: role.into(),
            states: Vec::new(),
            value: None,
            enabled: Some(true),
            hidden: Some(false),
            offscreen: Some(false),
        },
        states_complete: true,
        bounds: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 20.0,
        }),
        available_actions: actions.iter().map(|action| (*action).into()).collect(),
    }
}

fn refusal(role: &str, actions: &[&str], action: Action, no_pointer_session: bool) -> AdapterError {
    let handle = NativeHandle::null();
    let adapter = NoPointerSession;
    let hit_test = no_pointer_session.then_some((&handle, &adapter as &dyn PlatformAdapter));
    check_with_stability(
        StabilityExpectation::permissive(None),
        &evidence(role, actions),
        &ActionRequest::headless(action),
        hit_test,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap_err()
}

#[test]
fn type_on_a_field_with_set_value_names_set_value() {
    let error = refusal(
        "textfield",
        &["SetValue", "SetFocus"],
        Action::TypeText("x".into()),
        false,
    );
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    let hint = error.suggestion.unwrap();
    assert!(hint.contains("set-value"), "{hint}");
    assert!(!hint.contains("dropdown"), "{hint}");
}

#[test]
fn click_on_a_select_only_target_names_select() {
    let error = refusal("option", &["Select", "SetFocus"], Action::Click, false);
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(error.suggestion.unwrap().contains("'select'"));
}

#[test]
fn a_policy_refusal_never_blames_the_element_with_the_waiting_text() {
    let error = refusal("cell", &["SetFocus"], Action::Click, false);
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    let hint = error.suggestion.unwrap();
    assert!(!hint.contains("Waiting will not help"), "{hint}");
    assert!(!hint.contains("dropdown"), "{hint}");
    assert!(hint.contains("--headed"), "{hint}");
}

#[test]
fn a_session_without_pointer_input_does_not_offer_headed_mode() {
    let error = refusal("cell", &["SetFocus"], Action::Click, true);
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    let hint = error.suggestion.unwrap();
    assert!(hint.contains("headed mode cannot help"), "{hint}");
    assert!(!hint.contains("Use --headed"), "{hint}");
}
