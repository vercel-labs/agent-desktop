use serde_json::json;

use super::*;

fn look() -> Value {
    json!({"fill": [0.2, 0.6, 1.0], "rim": [0.0, 0.0, 0.0], "accent": [1.0, 0.5, 0.0], "size": 1.0})
}

#[test]
fn a_travel_request_is_parsed_with_its_owner_look_and_frames() {
    let request = json!({
        "op": "travel", "session": "s1", "agent": "a1", "look": look(), "label": "Agent",
        "frames": [[1.0, 2.0], [3.5, -4.0]], "frame_ms": 16.0, "pointer": true, "drag": true,
    });
    let Ok(Request::Travel {
        styled,
        path,
        pointer,
        clear_caret,
        drag,
    }) = parse_request(&request)
    else {
        panic!("travel parses");
    };
    assert_eq!(
        styled.owner,
        Owner {
            session: "s1".into(),
            agent: Some("a1".into())
        }
    );
    assert_eq!(styled.label.as_deref(), Some("Agent"));
    assert_eq!(styled.look.size, 1.0);
    assert_eq!(path.frames, vec![[1.0, 2.0], [3.5, -4.0]]);
    assert!(pointer);
    assert!(!clear_caret);
    assert!(drag);
}

#[test]
fn capture_requests_need_no_owner() {
    assert_eq!(
        parse_request(&json!({"op": "conceal"})),
        Ok(Request::Conceal)
    );
    assert_eq!(parse_request(&json!({"op": "reveal"})), Ok(Request::Reveal));
}

#[test]
fn an_unknown_operation_is_refused() {
    let error = parse_request(&json!({"op": "dance", "session": "s1"})).unwrap_err();
    assert!(
        error.starts_with("Invalid cursor request: op must be one of"),
        "{error}"
    );
}

#[test]
fn a_color_with_a_stray_entry_is_refused_even_when_three_entries_are_numbers() {
    let mut styled = look();
    styled["fill"] = json!([0.1, "x", 0.2, 0.3]);
    let request = json!({"op": "enable", "session": "s1", "look": styled});
    let error = parse_request(&request).unwrap_err();
    assert!(error.contains("fill must be three channels"), "{error}");
}

#[test]
fn a_blank_label_reads_as_no_label() {
    let request = json!({"op": "enable", "session": "s1", "look": look(), "label": "   "});
    let Ok(Request::Enable(styled)) = parse_request(&request) else {
        panic!("enable parses");
    };
    assert_eq!(styled.label, None);
}

#[test]
fn a_label_is_measured_in_utf16_units_like_the_extension() {
    let astral = "\u{1F600}".repeat(257);
    let request = json!({"op": "enable", "session": "s1", "look": look(), "label": astral});
    let error = parse_request(&request).unwrap_err();
    assert!(error.contains("label must be a string"), "{error}");
}

#[test]
fn effect_frames_outside_the_unit_range_are_refused() {
    let request = json!({
        "op": "effect", "session": "s1", "look": look(), "point": {"x": 1, "y": 2},
        "frames": [[0.5, 1.5]], "frame_ms": 16,
    });
    let error = parse_request(&request).unwrap_err();
    assert!(error.contains("every frame must be a pair"), "{error}");
}

#[test]
fn an_empty_travel_is_refused_but_an_empty_effect_is_not() {
    let travel =
        json!({"op": "travel", "session": "s1", "look": look(), "frames": [], "frame_ms": 16});
    assert!(
        parse_request(&travel)
            .unwrap_err()
            .contains("frames must hold 1 to 600")
    );
    let effect = json!({
        "op": "effect", "session": "s1", "look": look(), "point": {"x": 1, "y": 2},
        "target": {"x": 0, "y": 0, "width": 10, "height": 5}, "frames": [], "frame_ms": 16,
    });
    let Ok(Request::Effect { target, .. }) = parse_request(&effect) else {
        panic!("effect parses");
    };
    assert_eq!(
        target,
        Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 5.0
        })
    );
}

#[test]
fn a_relative_cursor_image_path_is_refused() {
    let mut styled = look();
    styled["arrow"] = json!({"path": "cursor.png", "hotspot": {"x": 0, "y": 0}});
    let request = json!({"op": "enable", "session": "s1", "look": styled});
    assert!(
        parse_request(&request)
            .unwrap_err()
            .contains("arrow needs an absolute path")
    );
}

#[test]
fn an_empty_agent_is_refused_and_a_missing_one_is_the_session_cursor() {
    let empty = json!({"op": "hide", "session": "s1", "agent": ""});
    assert!(parse_request(&empty).unwrap_err().contains("agent must be"));
    assert_eq!(
        parse_request(&json!({"op": "hide", "session": "s1"})),
        Ok(Request::Hide(Owner {
            session: "s1".into(),
            agent: None
        }))
    );
}

#[test]
fn a_locate_request_needs_a_finite_destination() {
    let located = parse_locate(&json!({"session": "s1", "destination": {"x": 4, "y": 5}}));
    assert_eq!(
        located,
        Ok(LocateRequest {
            owner: Owner {
                session: "s1".into(),
                agent: None
            },
            destination: Point { x: 4.0, y: 5.0 },
        })
    );
    assert!(parse_locate(&json!({"session": "s1", "destination": {"x": 4}})).is_err());
}

#[test]
fn frames_are_picked_by_elapsed_time_and_hold_the_last() {
    let path = Frames {
        frames: vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]],
        frame_ms: 10.0,
    };
    assert_eq!(path.length(), Duration::from_millis(20));
    assert_eq!(path.at(Duration::from_millis(0)), Some([0.0, 0.0]));
    assert_eq!(path.at(Duration::from_millis(19)), Some([1.0, 1.0]));
    assert_eq!(path.at(Duration::from_secs(5)), Some([2.0, 2.0]));
    let empty = Frames {
        frames: vec![],
        frame_ms: 10.0,
    };
    assert_eq!(empty.length(), Duration::ZERO);
    assert_eq!(empty.at(Duration::ZERO), None);
}
