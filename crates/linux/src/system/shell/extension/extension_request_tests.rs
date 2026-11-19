use super::*;
use agent_desktop_core::{CURSOR_OVERLAY_GREETING, CursorOverlayConfig};

fn config() -> CursorOverlayConfig {
    CursorOverlayConfig::enabled(Some("Saving the file".into()), 6).expect("enabled config")
}

fn instruction(x: f64, y: f64, click: bool) -> CursorOverlayInstruction {
    CursorOverlayInstruction::new(Point { x, y }, &config(), click).expect("instruction")
}

fn start(x: f64, y: f64, moves: u64) -> Option<CursorStart> {
    Some(CursorStart { x, y, moves })
}

fn image_at(path: &str, x: f64, y: f64) -> CursorImage {
    CursorImage::new(path.into(), Point { x, y }).expect("image")
}

fn present(instruction: CursorOverlayInstruction) -> CursorOverlayControl {
    CursorOverlayControl::present("s1".into(), instruction)
}

#[test]
fn enable_carries_the_greeting_and_the_whole_look() {
    let mut style = CursorOverlayStyle::default();
    style.set_size(1.5);
    let request = cursor_request(
        &CursorOverlayControl::enable("s1".into(), style.clone()),
        None,
    );
    assert_eq!(request["op"], "enable");
    assert_eq!(request["session"], "s1");
    assert_eq!(request["agent"], Value::Null);
    assert_eq!(request["label"], CURSOR_OVERLAY_GREETING);
    assert_eq!(request["look"]["size"], 1.5);
    assert_eq!(request["look"]["fill"], json!(style.fill_rgb()));
    assert_eq!(request["look"]["rim"], json!(style.rim_rgb()));
    assert_eq!(request["look"]["accent"], json!(style.accent_rgb()));
    assert_eq!(request["look"]["arrow"], Value::Null);
}

#[test]
fn a_custom_image_is_sent_with_its_hotspot_and_drops_the_greeting() {
    let mut style = CursorOverlayStyle::default();
    style.set_image(Some(image_at("/tmp/arrow.png", 3.0, 4.0)));
    style.set_pointer_image(Some(image_at("/tmp/hand.png", 9.0, 1.0)));
    let request = cursor_request(&CursorOverlayControl::enable("s1".into(), style), None);
    assert_eq!(
        request["look"]["arrow"],
        json!({"path": "/tmp/arrow.png", "hotspot": {"x": 3.0, "y": 4.0}})
    );
    assert_eq!(request["look"]["pointer"]["path"], "/tmp/hand.png");
    assert_eq!(request["label"], Value::Null);
}

#[test]
fn a_travel_plays_core_motion_from_the_reported_start_to_its_destination() {
    let control = present(instruction(418.0, 296.0, false)).with_agent_id(Some("a2".into()));
    let request = cursor_request(&control, start(18.0, 96.0, 3));
    assert_eq!(request["op"], "travel");
    assert_eq!(request["agent"], "a2");
    assert_eq!(request["label"], "Saving the file");
    let frames = request["frames"].as_array().expect("frames");
    assert_eq!(frames.first(), Some(&json!([18.0, 96.0])));
    assert_eq!(frames.last(), Some(&json!([418.0, 296.0])));
    assert_eq!(request["frame_ms"], json!(FRAME_MS));
    assert_eq!(request["pointer"], false);
    assert_eq!(request["drag"], false);
}

#[test]
fn a_seeded_profile_varies_the_path_with_the_move_count() {
    let mut profile = CursorMotionProfile::default();
    profile.set_seed(Some(7));
    let control = present(instruction(900.0, 500.0, false)).with_motion(profile);
    let first = cursor_request(&control, start(100.0, 100.0, 0));
    let again = cursor_request(&control, start(100.0, 100.0, 0));
    let second = cursor_request(&control, start(100.0, 100.0, 1));
    assert_eq!(first["frames"], again["frames"]);
    assert_ne!(first["frames"], second["frames"]);
}

#[test]
fn a_travel_away_clears_the_caret_and_one_that_stays_put_keeps_it() {
    let control = present(instruction(300.0, 200.0, false));
    let away = cursor_request(&control, start(10.0, 10.0, 0));
    let staying = cursor_request(&control, start(300.5, 200.0, 0));
    assert_eq!(away["clear_caret"], true);
    assert_eq!(staying["clear_caret"], false);
}

#[test]
fn a_drag_travels_to_where_it_begins_and_says_it_is_a_drag() {
    let drag = instruction(700.0, 500.0, false)
        .with_drag_from(Some(Point { x: 120.0, y: 80.0 }))
        .with_phase(CursorPhase::Drag);
    let request = cursor_request(&present(drag), start(10.0, 10.0, 0));
    assert_eq!(request["op"], "travel");
    assert_eq!(
        request["frames"].as_array().and_then(|f| f.last()),
        Some(&json!([120.0, 80.0]))
    );
    assert_eq!(request["drag"], true);
}

#[test]
fn a_click_effect_plays_the_ripple_and_outlines_its_target() {
    let target = Rect {
        x: 50.0,
        y: 60.0,
        width: 120.0,
        height: 40.0,
    };
    let effect = instruction(110.0, 80.0, true)
        .with_target(Some(target))
        .with_phase(CursorPhase::Effect);
    let request = cursor_request(&present(effect), None);
    assert_eq!(request["op"], "effect");
    assert_eq!(request["point"], json!({"x": 110.0, "y": 80.0}));
    assert_eq!(
        request["target"],
        json!({"x": 50.0, "y": 60.0, "width": 120.0, "height": 40.0})
    );
    let frames = request["frames"].as_array().expect("frames");
    assert!(frames.iter().any(|frame| frame[0].as_f64() > Some(0.0)));
    assert!(frames.iter().any(|frame| frame[1] == json!(1.0)));
}

#[test]
fn switched_off_effects_send_no_ripple_and_no_outline() {
    let mut style = CursorOverlayStyle::default();
    style.set_effects(false, false);
    let effect = instruction(110.0, 80.0, true)
        .with_target(Some(Rect {
            x: 50.0,
            y: 60.0,
            width: 120.0,
            height: 40.0,
        }))
        .with_phase(CursorPhase::Effect);
    let control = CursorOverlayControl::present_with_style("s1".into(), effect, style);
    let request = cursor_request(&control, None);
    assert_eq!(request["target"], Value::Null);
    assert_eq!(request["frames"], json!([]));
}

#[test]
fn a_caret_marks_its_target_and_a_follow_says_so() {
    let caret = Rect {
        x: 40.0,
        y: 50.0,
        width: 1.0,
        height: 18.0,
    };
    for (phase, follow) in [
        (CursorPhase::Caret, false),
        (CursorPhase::CaretFollow, true),
    ] {
        let marked = instruction(40.0, 59.0, false)
            .with_target(Some(caret))
            .with_phase(phase);
        let request = cursor_request(&present(marked), None);
        assert_eq!(request["op"], "caret");
        assert_eq!(
            request["rect"],
            json!({"x": 40.0, "y": 50.0, "width": 1.0, "height": 18.0})
        );
        assert_eq!(request["follow"], follow);
        assert!(request.get("frames").is_none(), "{request}");
    }
}

#[test]
fn hide_and_show_without_an_agent_reach_every_cursor_of_the_session() {
    let hide = cursor_request(&CursorOverlayControl::hide("s1".into()), None);
    let show = cursor_request(&CursorOverlayControl::show("s1".into()), None);
    assert_eq!(hide, json!({"op": "hide", "session": "s1", "agent": null}));
    assert_eq!(show, json!({"op": "show", "session": "s1", "agent": null}));
}

#[test]
fn hide_with_an_agent_names_that_agent() {
    let control = CursorOverlayControl::hide("s1".into()).with_agent_id(Some("a2".into()));
    assert_eq!(
        cursor_request(&control, None),
        json!({"op": "hide", "session": "s1", "agent": "a2"})
    );
}

#[test]
fn disable_removes_the_whole_session() {
    assert_eq!(
        cursor_request(&CursorOverlayControl::disable("s1".into()), None),
        json!({"op": "disable", "session": "s1"})
    );
}
