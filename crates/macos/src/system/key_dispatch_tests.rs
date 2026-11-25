use super::*;
use agent_desktop_core::Modifier;

#[test]
fn process_preflight_failure_reports_no_key_delivery() {
    let error = press_for_app_impl(
        ProcessIdentity::new(u32::MAX, "invalid"),
        &combo(Vec::new()),
        agent_desktop_core::InteractionPolicy::default(),
        Deadline::standard().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

fn combo(modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: "a".into(),
        modifiers,
    }
}

fn key(name: &str, modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: name.into(),
        modifiers,
    }
}

#[test]
fn printable_keys_are_never_translated_into_ax_actions() {
    for name in ["space", "a", "1", "tab", "delete"] {
        assert_eq!(simple_key_action(&key(name, Vec::new())), None, "{name}");
    }
}

#[test]
fn return_and_escape_map_to_default_and_cancel_actions() {
    assert_eq!(
        simple_key_action(&key("return", Vec::new())),
        Some("AXConfirm")
    );
    assert_eq!(
        simple_key_action(&key("enter", Vec::new())),
        Some("AXConfirm")
    );
    assert_eq!(
        simple_key_action(&key("escape", Vec::new())),
        Some("AXCancel")
    );
    assert_eq!(simple_key_action(&key("esc", Vec::new())), Some("AXCancel"));
    assert_eq!(
        simple_key_action(&key("return", vec![Modifier::Meta])),
        None
    );
}

#[test]
fn headless_press_never_performs_menu_shortcuts() {
    let headless = agent_desktop_core::InteractionPolicy::headless();
    assert!(!uses_menu_shortcut(
        &key("n", vec![Modifier::Meta]),
        headless
    ));
    assert!(!uses_menu_shortcut(
        &key("n", vec![Modifier::Meta, Modifier::Shift]),
        headless
    ));
}

#[test]
fn focus_authorized_press_keeps_menu_shortcuts_for_modified_keys() {
    let headed = agent_desktop_core::InteractionPolicy::headed();
    assert!(uses_menu_shortcut(&key("n", vec![Modifier::Meta]), headed));
    assert!(!uses_menu_shortcut(&key("n", Vec::new()), headed));
}

#[test]
fn a_missing_focused_element_names_both_recovery_routes() {
    let error = super::no_focused_element_error(agent_desktop_core::InteractionPolicy::headless());
    let suggestion = error
        .suggestion
        .expect("the error carries a recovery suggestion");
    assert!(suggestion.contains("--headed"), "{suggestion}");
    assert!(suggestion.contains("--surface menubar"), "{suggestion}");
}

#[test]
fn a_missing_focused_element_is_a_refusal_core_can_route_to_a_surface_close() {
    let error = super::no_focused_element_error(agent_desktop_core::InteractionPolicy::headless());

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("refusal details");
    assert_eq!(details["check"], "process_focus");
    assert_eq!(details["physical_delivery_started"], false);
}

#[test]
fn a_headed_missing_focused_element_does_not_suggest_enabling_headed() {
    let error = super::no_focused_element_error(agent_desktop_core::InteractionPolicy::headed());
    let hint = error.suggestion.expect("recovery suggestion");
    assert!(!hint.contains("--headed"), "{hint}");
    assert!(hint.contains("--surface menubar"), "{hint}");
}

#[test]
fn an_unresolvable_key_fails_before_any_focus_change() {
    let pid = std::process::id();
    let instance = crate::system::process::process_identity::token_for_pid(pid as i32)
        .unwrap()
        .unwrap();
    let error = press_for_app_impl(
        ProcessIdentity::new(pid, instance),
        &key("comma", Vec::new()),
        agent_desktop_core::InteractionPolicy::headed(),
        Deadline::standard().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(
        error.message.contains("keyboard layout"),
        "{}",
        error.message
    );
}

#[test]
fn escape_reaches_the_focus_holder_first_then_the_open_surface() {
    use super::{SemanticKeyTarget, choose_key_target};

    assert_eq!(
        choose_key_target(Some(1), "AXCancel", || panic!("surface read")).unwrap(),
        Some(SemanticKeyTarget::FocusHolder(1))
    );
    assert_eq!(
        choose_key_target(None, "AXCancel", || Ok(Some(2))).unwrap(),
        Some(SemanticKeyTarget::OpenSurface(2))
    );
    assert_eq!(
        choose_key_target::<u8>(None, "AXCancel", || Ok(None)).unwrap(),
        None
    );
    assert_eq!(
        choose_key_target::<u8>(None, "AXConfirm", || panic!("surface read")).unwrap(),
        None
    );
}

#[test]
fn a_failed_surface_read_is_not_turned_into_an_absent_surface() {
    use super::choose_key_target;

    let error = choose_key_target::<u8>(None, "AXCancel", || {
        Err(AdapterError::stale_ref_because(
            "Application process instance changed during observation",
        ))
    })
    .expect_err("a failed surface read must not read as no surface");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_command_chord_to_a_background_app_is_refused_before_sending() {
    let headless = agent_desktop_core::InteractionPolicy::headless();
    let chord = key("s", vec![Modifier::Meta]);
    assert!(chord_needs_frontmost(&chord, headless));
    let refusal = background_chord_refusal(&chord, false).expect("refused");
    assert_eq!(refusal.code, ErrorCode::ActionFailed);
    assert_eq!(refusal.disposition, DeliverySemantics::not_delivered());
    assert_eq!(refusal.details.as_ref().unwrap()["check"], "process_focus");
}

#[test]
fn the_same_chord_to_the_front_app_is_sent() {
    assert!(background_chord_refusal(&key("s", vec![Modifier::Meta]), true).is_none());
}

#[test]
fn plain_and_shift_keys_and_headed_delivery_do_not_need_the_front_app() {
    let headless = agent_desktop_core::InteractionPolicy::headless();
    assert!(!chord_needs_frontmost(&key("q", Vec::new()), headless));
    assert!(!chord_needs_frontmost(
        &key("left", vec![Modifier::Shift]),
        headless
    ));
    assert!(!chord_needs_frontmost(
        &key("s", vec![Modifier::Meta]),
        agent_desktop_core::InteractionPolicy::headed()
    ));
}
