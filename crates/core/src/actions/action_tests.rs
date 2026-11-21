use super::Action;
use crate::{
    Direction, DragParams, KeyCombo, Modifier, MouseButton, MouseEvent, MouseEventKind, Point,
};

fn dummy_key() -> KeyCombo {
    KeyCombo {
        key: "a".into(),
        modifiers: vec![],
    }
}

fn dummy_drag() -> DragParams {
    DragParams {
        from: Point { x: 0.0, y: 0.0 },
        to: Point { x: 1.0, y: 1.0 },
        duration_ms: None,
        drop_delay_ms: None,
    }
}

#[test]
fn action_names_do_not_include_payloads() {
    let cases = [
        (Action::SetValue("private".into()), "set-value"),
        (Action::Select("private".into()), "select"),
        (Action::TypeText("private".into()), "type"),
        (
            Action::PressKey(KeyCombo {
                key: "A".into(),
                modifiers: vec![Modifier::Meta],
            }),
            "press",
        ),
    ];

    for (action, expected) in cases {
        assert_eq!(action.name(), expected);
    }
}

#[test]
fn headed_focuss_distinguish_window_and_pointer_preconditions() {
    for action in [
        Action::Click,
        Action::DoubleClick,
        Action::RightClick,
        Action::TripleClick,
        Action::Scroll(Direction::Down, 1),
        Action::Hover,
        Action::Drag(dummy_drag()),
    ] {
        assert_eq!(
            action.headed_focus(),
            crate::HeadedFocus::WindowAndCursor,
            "{} must require focus plus cursor delivery",
            action.name()
        );
    }
    for action in [
        Action::SetValue("v".into()),
        Action::SetFocus,
        Action::Expand,
        Action::Collapse,
        Action::Select("v".into()),
        Action::Toggle,
        Action::Check,
        Action::Uncheck,
        Action::ScrollTo,
        Action::PressKey(dummy_key()),
        Action::TypeText("v".into()),
        Action::Clear,
    ] {
        assert_eq!(
            action.headed_focus(),
            crate::HeadedFocus::Window,
            "{} must require focus without pointer movement",
            action.name()
        );
    }
}

#[test]
fn requires_hit_test_covers_ref_targeted_pointer_actions() {
    let hit_tested: &[Action] = &[
        Action::Click,
        Action::DoubleClick,
        Action::RightClick,
        Action::TripleClick,
        Action::Hover,
        Action::Drag(dummy_drag()),
    ];
    for action in hit_tested {
        assert!(
            action.requires_hit_test(),
            "{} must require a hit test before dispatch",
            action.name()
        );
    }

    let not_hit_tested: &[Action] = &[
        Action::SetValue("v".into()),
        Action::SetFocus,
        Action::Expand,
        Action::Collapse,
        Action::Select("s".into()),
        Action::Toggle,
        Action::Check,
        Action::Uncheck,
        Action::Scroll(Direction::Down, 1),
        Action::ScrollTo,
        Action::PressKey(dummy_key()),
        Action::KeyDown(dummy_key()),
        Action::KeyUp(dummy_key()),
        Action::TypeText("t".into()),
        Action::Clear,
    ];
    for action in not_hit_tested {
        assert!(
            !action.requires_hit_test(),
            "{} must not require a hit test",
            action.name()
        );
    }
}

#[test]
fn requires_scroll_into_view_covers_all_actions() {
    let scroll_into_view: &[Action] = &[
        Action::Click,
        Action::DoubleClick,
        Action::RightClick,
        Action::TripleClick,
        Action::SetValue("v".into()),
        Action::Expand,
        Action::Collapse,
        Action::Select("s".into()),
        Action::Toggle,
        Action::Check,
        Action::Uncheck,
        Action::TypeText("t".into()),
        Action::Clear,
        Action::Hover,
        Action::Drag(dummy_drag()),
        Action::Scroll(Direction::Down, 1),
    ];
    for action in scroll_into_view {
        assert!(
            action.requires_scroll_into_view(),
            "{} must require scroll-into-view before dispatch",
            action.name()
        );
    }

    let not_scroll_into_view: &[Action] = &[
        Action::ScrollTo,
        Action::SetFocus,
        Action::PressKey(dummy_key()),
        Action::KeyDown(dummy_key()),
        Action::KeyUp(dummy_key()),
    ];
    for action in not_scroll_into_view {
        assert!(
            !action.requires_scroll_into_view(),
            "{} must not require scroll-into-view",
            action.name()
        );
    }
}

/// F10 regression coverage: `MouseEvent.modifiers` must stay `#[serde(default)]`
/// so a legacy payload recorded before modifiers existed (or any FFI/batch
/// caller that omits the key) still deserializes instead of erroring out.
#[test]
fn mouse_event_json_without_modifiers_key_deserializes_to_empty() {
    let event: MouseEvent = serde_json::from_value(serde_json::json!({
        "kind": { "Click": { "count": 1 } },
        "point": { "x": 1.0, "y": 2.0 },
        "button": "Left",
    }))
    .unwrap();

    assert!(event.modifiers.is_empty());
}

#[test]
fn mouse_event_json_with_modifiers_round_trips() {
    let event = MouseEvent {
        kind: MouseEventKind::Click { count: 2 },
        point: Point { x: 1.0, y: 2.0 },
        button: MouseButton::Left,
        modifiers: vec![Modifier::Meta, Modifier::Shift],
    };

    let json = serde_json::to_value(&event).unwrap();
    let round_tripped: MouseEvent = serde_json::from_value(json).unwrap();

    assert_eq!(
        round_tripped.modifiers,
        vec![Modifier::Meta, Modifier::Shift]
    );
}

#[test]
fn modifier_meta_serializes_semantically_and_accepts_legacy_cmd() {
    assert_eq!(serde_json::to_string(&Modifier::Meta).unwrap(), "\"Meta\"");
    assert_eq!(
        serde_json::from_str::<Modifier>("\"Cmd\"").unwrap(),
        Modifier::Meta
    );
}

#[test]
fn wheel_event_round_trips_platform_neutral_line_deltas() {
    let event = MouseEvent {
        kind: MouseEventKind::Wheel {
            delta_x: -2.0,
            delta_y: 3.0,
        },
        point: Point { x: 50.0, y: 60.0 },
        button: MouseButton::Left,
        modifiers: vec![Modifier::Shift],
    };
    let json = serde_json::to_value(&event).unwrap();
    let decoded: MouseEvent = serde_json::from_value(json).unwrap();
    assert!(matches!(
        decoded.kind,
        MouseEventKind::Wheel {
            delta_x: -2.0,
            delta_y: 3.0
        }
    ));
}
