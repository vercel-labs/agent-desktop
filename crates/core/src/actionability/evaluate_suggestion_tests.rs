use super::*;
use crate::{Action, Rect};

fn select_error(role: &str) -> AdapterError {
    let evidence = ActionabilityEvidence {
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
        available_actions: Vec::new(),
    };
    check_with_stability(
        StabilityExpectation::permissive(None),
        &evidence,
        &ActionRequest::headless(Action::Select("Option B".into())),
        None,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap_err()
}

fn click_error(enabled: Option<bool>, offscreen: Option<bool>) -> AdapterError {
    let evidence = ActionabilityEvidence {
        nonspatial: false,
        state: crate::ElementState {
            role: "button".into(),
            states: Vec::new(),
            value: None,
            enabled,
            hidden: Some(false),
            offscreen,
        },
        states_complete: true,
        bounds: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 20.0,
        }),
        available_actions: vec!["Click".into()],
    };
    check_with_stability(
        StabilityExpectation::permissive(None),
        &evidence,
        &ActionRequest::headless(Action::Click),
        None,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap_err()
}

#[test]
fn select_unsupported_on_an_item_collection_names_the_item_to_click() {
    let error = select_error("listbox");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    let suggestion = error.suggestion.unwrap();
    assert!(
        suggestion.starts_with(
            "To choose one item, click the item itself: find it by name in a fresh snapshot and click its ref."
        ),
        "{suggestion}"
    );
    assert!(suggestion.contains("Waiting will not help"), "{suggestion}");
}

#[test]
fn select_unsupported_off_an_item_collection_keeps_the_generic_hint() {
    let error = select_error("button");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(
        error.suggestion.as_deref(),
        Some(
            "Waiting will not help: this element cannot satisfy the action as targeted. Target an element that advertises the action (check available_actions in a fresh snapshot). For a dropdown without select support, open it with click, inspect its choices, and use explicit keyboard input."
        )
    );
}

#[test]
fn an_offscreen_only_visibility_failure_suggests_scroll_to() {
    let error = click_error(Some(true), Some(true));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(
        error
            .message
            .contains("visible (live offscreen state is true)"),
        "{}",
        error.message
    );
    let suggestion = error.suggestion.unwrap();
    assert!(
        suggestion.contains("Run 'scroll-to' on its ref first"),
        "{suggestion}"
    );
    assert!(
        suggestion.contains("only if the user explicitly asked"),
        "{suggestion}"
    );
}

#[test]
fn a_not_offscreen_action_failure_keeps_the_wait_suggestion() {
    let error = click_error(Some(false), Some(false));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(
        error
            .message
            .contains("enabled (live enabled state is false)"),
        "{}",
        error.message
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some(
            "Wait for the target to become actionable, refresh the snapshot, or use an \
             explicit physical/focus command if intended."
        )
    );
}

#[test]
fn a_disabled_target_off_screen_keeps_the_wait_suggestion() {
    let error = click_error(Some(false), Some(true));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.suggestion.as_deref(),
        Some(
            "Wait for the target to become actionable, refresh the snapshot, or use an \
             explicit physical/focus command if intended."
        ),
        "scrolling cannot enable a disabled target"
    );
}
