use super::execute_action_impl;
use agent_desktop_core::{
    Action, ActionRequest, Deadline, ErrorCode, InteractionLease, KeyCombo, NativeHandle,
    StepMechanism,
};

fn lease() -> InteractionLease {
    InteractionLease::guarded(Deadline::after(5_000).expect("deadline"), ()).expect("lease")
}

fn dummy_key() -> KeyCombo {
    KeyCombo {
        key: "a".into(),
        modifiers: vec![],
    }
}

fn code_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("///") || trimmed.starts_with("//!"))
        })
        .map(|(index, line)| (index + 1, line))
}

#[test]
fn null_handle_press_key_synthesizes_globally() {
    use crate::input::keyboard_send_fake_sink as key_sink;
    key_sink::reset();

    let result = execute_action_impl(
        &NativeHandle::null(),
        ActionRequest::headless(Action::PressKey(dummy_key())),
        &lease(),
    )
    .expect("null PressKey synthesizes");

    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.steps[0].label(), "SendInput.press_key");
    assert_eq!(
        result.steps[0].mechanism(),
        Some(StepMechanism::PhysicalSynthetic)
    );
    assert!(!key_sink::recorded().is_empty());
}

#[test]
fn null_handle_click_is_invalid_native_handle() {
    let error = execute_action_impl(
        &NativeHandle::null(),
        ActionRequest::headless(Action::Click),
        &lease(),
    )
    .expect_err("null Click");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.details.as_ref().and_then(|d| d.get("kind")),
        Some(&serde_json::json!("invalid_native_handle"))
    );
}

#[test]
fn set_focus_call_site_lives_only_in_focus_rs() {
    let sources = crate::actions::mutation::action_scan_sources();
    let banned = concat!(".", "set_focus(");
    for (name, source) in sources {
        for (number, line) in code_lines(source) {
            if name.ends_with("focus.rs") {
                continue;
            }
            assert!(
                !line.contains(banned),
                "{name}:{number} must not call set_focus: {line}"
            );
        }
    }
    assert!(
        include_str!("focus.rs").contains(banned),
        "focus.rs must own the set_focus call site"
    );
}
