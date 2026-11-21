use agent_desktop_core::{Action, DeliverySemantics, ErrorCode, InteractionPolicy};

use super::{Effect, PressTarget, action_effect};

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
            Some(Effect::ShowsMenu)
        );
        assert_eq!(
            effect(Action::Expand, target(role, "View")),
            Some(Effect::ShowsMenu)
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
                Some(Effect::OpensDialog)
            );
        }
    }
    assert_eq!(effect(Action::Click, None), Some(Effect::OpensDialog));
    assert_eq!(
        action_effect(&Action::RightClick, || panic!("known from the action")),
        Some(Effect::ShowsMenu)
    );
    for role in ["AXButton", "AXRadioButton", "AXMenuBarItem", "AXMenuItem"] {
        assert_eq!(effect(Action::Click, target(role, "Week")), None, "{role}");
    }
    for action in [Action::SetFocus, Action::Clear, Action::ScrollTo] {
        assert_eq!(action_effect(&action, || panic!("never read")), None);
    }
}

#[test]
fn a_refusal_is_policy_denied_not_delivered_and_names_its_effect() {
    for effect in [
        Effect::ShowsMenu,
        Effect::OpensDialog,
        Effect::TakesKeyFocus,
        Effect::PhysicalInput,
    ] {
        let refusal = effect
            .refuse_unless_permitted(InteractionPolicy::headless())
            .unwrap_err();
        assert_eq!(refusal.code, ErrorCode::PolicyDenied);
        assert_eq!(refusal.disposition, DeliverySemantics::not_delivered());
        assert_eq!(
            refusal.details.as_ref().unwrap()["foreground_effect"],
            serde_json::json!(true)
        );
        assert!(
            effect
                .refuse_unless_permitted(InteractionPolicy::headed())
                .is_ok()
        );
    }
    let focus = InteractionPolicy::focus_fallback();
    assert!(Effect::ShowsMenu.permitted(focus));
    assert!(!Effect::PhysicalInput.permitted(focus));
}
