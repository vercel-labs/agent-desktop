use agent_desktop_core::{Action, DeliverySemantics, Effect, ErrorCode, InteractionPolicy};

use super::{PressTarget, action_effect, require};

fn target(role: &str, label: &str) -> Option<PressTarget> {
    Some(PressTarget {
        role: Some(role.into()),
        labels: vec![label.into()],
    })
}

#[test]
fn headless_refuses_what_the_user_would_see_and_delivers_the_rest() {
    let effect = |action: Action, read: Option<PressTarget>| action_effect(&action, || read);
    for role in ["AXPopUpButton", "AXMenuButton", "AXComboBox"] {
        assert_eq!(
            effect(Action::Click, target(role, "View")),
            Some("shows_menu")
        );
        assert_eq!(
            effect(Action::Expand, target(role, "View")),
            Some("shows_menu")
        );
    }
    for label in [
        "New Calendar Subscription\u{2026}",
        "Save As...",
        "Export\u{2026} ",
    ] {
        for action in [
            Action::Click,
            Action::Toggle,
            Action::Check,
            Action::Uncheck,
        ] {
            assert_eq!(
                effect(action, target("AXMenuItem", label)),
                Some("opens_dialog")
            );
        }
    }
    assert_eq!(effect(Action::Click, None), Some("opens_dialog"));
    assert_eq!(
        action_effect(&Action::RightClick, || panic!("known from the action")),
        Some("shows_menu")
    );
    for role in ["AXButton", "AXRadioButton", "AXMenuBarItem", "AXMenuItem"] {
        assert_eq!(effect(Action::Click, target(role, "Week")), None, "{role}");
    }
    for action in [Action::SetFocus, Action::Clear, Action::ScrollTo] {
        assert_eq!(action_effect(&action, || panic!("never read")), None);
    }
}

#[test]
fn a_refusal_is_policy_denied_not_delivered_and_names_its_kind() {
    for (effect, kind) in [
        (Effect::FocusSteal, "shows_menu"),
        (Effect::CursorMove, "physical_input"),
    ] {
        let refusal = require(InteractionPolicy::headless(), effect, "select", kind).unwrap_err();
        assert_eq!(refusal.code, ErrorCode::PolicyDenied);
        assert_eq!(refusal.disposition, DeliverySemantics::not_delivered());
        let details = refusal.details.as_ref().unwrap();
        assert_eq!(details["foreground_effect"], serde_json::json!(true));
        assert_eq!(details["kind"], serde_json::json!(kind));
        assert_eq!(details["effect"], serde_json::json!(effect));
        assert!(require(InteractionPolicy::headed(), effect, "select", kind).is_ok());
    }
    let focus_only = InteractionPolicy::focus_fallback();
    assert!(require(focus_only, Effect::FocusSteal, "select", "shows_menu").is_ok());
    assert!(require(focus_only, Effect::CursorMove, "select", "physical_input").is_err());
}
