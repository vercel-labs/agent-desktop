use super::*;
use agent_desktop_core::{CURSOR_MOTION_BUDGET_MS, Point};

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[test]
fn a_travel_starts_on_its_origin_and_ends_on_its_destination() {
    let motion = CursorMotion::new(point(20.0, 40.0), point(620.0, 340.0));
    let frames = travel_frames(&motion);
    assert_eq!(frames.first(), Some(&[20.0, 40.0]));
    assert_eq!(frames.last(), Some(&[620.0, 340.0]));
    assert!(frames.len() > 2, "{frames:?}");
}

#[test]
fn a_travel_moves_through_intermediate_points_rather_than_jumping() {
    let motion = CursorMotion::new(point(0.0, 0.0), point(800.0, 0.0));
    let frames = travel_frames(&motion);
    let middle = frames[frames.len() / 2];
    assert!(middle[0] > 0.0 && middle[0] < 800.0, "{middle:?}");
}

#[test]
fn a_travel_fits_inside_the_arrival_budget() {
    let motion = CursorMotion::new(point(0.0, 0.0), point(4_000.0, 3_000.0));
    let frames = travel_frames(&motion);
    let played_ms = (frames.len() - 1) as f64 * FRAME_MS;
    assert!(
        played_ms <= CURSOR_MOTION_BUDGET_MS as f64 + FRAME_MS,
        "{played_ms}"
    );
}

#[test]
fn a_travel_to_where_the_cursor_stands_is_a_single_frame_plus_its_dwell() {
    let motion = CursorMotion::new(point(5.0, 5.0), point(5.0, 5.0));
    let frames = travel_frames(&motion);
    assert!(
        frames.iter().all(|frame| *frame == [5.0, 5.0]),
        "{frames:?}"
    );
    assert_eq!(
        frames.len() as u64,
        (motion.total_ms() as f64 / FRAME_MS).ceil() as u64 + 1
    );
}

#[test]
fn a_click_ripple_rises_from_zero_to_its_end() {
    let at = point(100.0, 100.0);
    let motion = CursorMotion::new(at.clone(), at)
        .with_impact(true)
        .with_ripple(true);
    let frames = effect_frames(&motion, true, false);
    assert_eq!(frames.first().map(|frame| frame[0]), Some(0.0));
    assert_eq!(frames.last().map(|frame| frame[0]), Some(1.0));
    assert!(frames.iter().all(|frame| frame[1] == 0.0));
}

#[test]
fn a_highlight_rises_holds_and_falls_back_to_nothing() {
    let at = point(100.0, 100.0);
    let motion = CursorMotion::new(at.clone(), at);
    let frames = effect_frames(&motion, false, true);
    let opacities: Vec<f64> = frames.iter().map(|frame| frame[1]).collect();
    assert_eq!(opacities.first(), Some(&0.0));
    assert_eq!(opacities.last(), Some(&0.0));
    assert!(opacities.contains(&1.0), "{opacities:?}");
    assert!(frames.iter().all(|frame| frame[0] == 0.0));
}

#[test]
fn an_effect_with_neither_a_ripple_nor_a_highlight_plays_nothing() {
    let at = point(100.0, 100.0);
    assert!(effect_frames(&CursorMotion::new(at.clone(), at), false, false).is_empty());
}

#[test]
fn the_highlight_envelope_is_zero_outside_its_hold() {
    assert_eq!(highlight_progress(0, 900), 0.0);
    assert_eq!(highlight_progress(450, 900), 1.0);
    assert_eq!(highlight_progress(900, 900), 0.0);
    assert_eq!(highlight_progress(10, 0), 0.0);
    assert!((highlight_progress(36, 900) - 0.5).abs() < 1e-9);
}
