use super::*;
use crate::{Action, InteractionPolicy, Rect};

fn unsupported(action: Action, policy: InteractionPolicy) -> AdapterError {
    let evidence = ActionabilityEvidence {
        nonspatial: false,
        state: crate::ElementState {
            role: "textfield".into(),
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
        available_actions: Vec::new(),
    };
    let mut request = ActionRequest::headless(action);
    request.policy = policy;
    let error = check_with_stability(
        StabilityExpectation::permissive(None),
        &evidence,
        &request,
        None,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    error
}

#[test]
fn unsupported_focus_names_a_supported_target_instead_of_repeating_headed() {
    for policy in [InteractionPolicy::headless(), InteractionPolicy::headed()] {
        let hint = unsupported(Action::SetFocus, policy).suggestion.unwrap();
        assert!(hint.contains("SetFocus"), "{hint}");
        assert!(hint.contains("focus-window"), "{hint}");
        assert!(!hint.contains("--headed"), "{hint}");
    }
}

#[test]
fn unsupported_text_write_names_typing_for_the_actual_mode() {
    for action in [Action::SetValue("search".into()), Action::Clear] {
        for policy in [
            InteractionPolicy::headless(),
            InteractionPolicy::focus_fallback(),
            InteractionPolicy::headed(),
        ] {
            let hint = unsupported(action.clone(), policy).suggestion.unwrap();
            assert_eq!(
                hint.contains("only if the user explicitly asked"),
                !policy.is_headed(),
                "{hint}"
            );
            assert_eq!(
                hint.contains("current headed mode"),
                policy.is_headed(),
                "{hint}"
            );
            if policy.is_headed() {
                assert!(hint.contains("clipboard-set"), "{hint}");
                assert!(hint.contains("paste shortcut"), "{hint}");
            }
        }
    }
}

struct ForegroundAdapter;

impl crate::adapter::ObservationOps for ForegroundAdapter {}
impl crate::adapter::InputOps for ForegroundAdapter {}
impl crate::adapter::SystemOps for ForegroundAdapter {}
impl crate::adapter::ActionOps for ForegroundAdapter {
    fn foreground_effect(
        &self,
        _handle: &NativeHandle,
        _request: &ActionRequest,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::DeclaredEffect>, AdapterError> {
        Ok(Some(crate::DeclaredEffect {
            effect: crate::Effect::FocusSteal,
            kind: "opens_popup",
        }))
    }

    fn headed_focus(
        &self,
        _handle: &NativeHandle,
        _request: &ActionRequest,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::HeadedFocus>, AdapterError> {
        Ok(Some(crate::HeadedFocus::Application))
    }
}

fn button(hidden: bool) -> ActionabilityEvidence {
    ActionabilityEvidence {
        nonspatial: false,
        state: crate::ElementState {
            role: "button".into(),
            states: Vec::new(),
            value: None,
            enabled: Some(true),
            hidden: Some(hidden),
            offscreen: Some(false),
        },
        states_complete: true,
        bounds: None,
        available_actions: vec!["Click".into()],
    }
}

fn checked(request: ActionRequest, hidden: bool) -> Result<ActionabilityReport, AdapterError> {
    let handle = NativeHandle::null();
    check_with_stability(
        StabilityExpectation::permissive(None),
        &button(hidden),
        &request,
        Some((&handle, &ForegroundAdapter)),
        crate::Deadline::standard().unwrap(),
    )
}

/// A hidden button fails visibility; a headless request is refused for the
/// foreground alone, before it. A policy that allows focus is not refused.
#[test]
fn a_foreground_the_policy_forbids_is_the_only_refusal_and_comes_first() {
    let error = checked(ActionRequest::headless(Action::Click), true).unwrap_err();
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    let details = error.details.unwrap();
    assert_eq!(details["check"], "foreground");
    assert_eq!(details["effect"], "focus_steal");
    assert_eq!(details["kind"], "opens_popup");
    assert_eq!(details["foreground_effect"], true);
    assert_eq!(details["mechanisms"], serde_json::json!(["click"]));
    assert!(details.get("checks").is_none(), "{details}");
    assert!(error.suggestion.unwrap().contains("--headed"));
    for request in [
        ActionRequest::headed(Action::Click),
        ActionRequest::focus_fallback(Action::Click),
    ] {
        let result = checked(request, false);
        assert!(result.is_ok(), "{result:?}");
    }
}

#[test]
fn a_raw_dispatch_passes_the_same_foreground_gate() {
    let handle = NativeHandle::null();
    let deadline = crate::Deadline::standard().unwrap();
    let gate =
        |request| crate::ensure_foreground_allowed(&ForegroundAdapter, &handle, &request, deadline);
    let error = gate(ActionRequest::headless(Action::SetFocus)).unwrap_err();
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(error.details.unwrap()["check"], "foreground");
    assert!(gate(ActionRequest::focus_fallback(Action::SetFocus)).is_ok());
}
