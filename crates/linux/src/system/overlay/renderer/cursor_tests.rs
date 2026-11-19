use std::time::{Duration, Instant};

use super::*;
use crate::system::overlay::renderer::presence::REST_FADE;
use crate::system::overlay::renderer::request::Owner;

struct StillDesk;

impl Desk for StillDesk {
    fn pointer(&self) -> Point {
        Point { x: 40.0, y: 30.0 }
    }

    fn monitor_at(&self, _at: &Point) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    fn work_area_at(&self, at: &Point) -> Rect {
        self.monitor_at(at)
    }
}

fn look() -> Look {
    Look {
        fill: [0.2, 0.6, 1.0],
        rim: [0.0; 3],
        accent: [1.0, 0.5, 0.0],
        size: 1.0,
        arrow: None,
        pointer: None,
    }
}

fn styled(label: Option<&str>) -> Styled {
    Styled {
        owner: Owner {
            session: "s1".into(),
            agent: None,
        },
        look: look(),
        label: label.map(str::to_owned),
    }
}

fn order(frames: Vec<[f64; 2]>, ticket: Ticket) -> TravelOrder {
    TravelOrder {
        path: Frames {
            frames,
            frame_ms: 10.0,
        },
        pointer: true,
        clear_caret: false,
        drag: false,
        ticket,
    }
}

fn ms(start: Instant, millis: u64) -> Instant {
    start + Duration::from_millis(millis)
}

#[test]
fn enabling_places_an_unplaced_cursor_at_the_real_pointer() {
    let now = Instant::now();
    let mut cursor = Cursor::new(look(), now, false);
    cursor.enable(&styled(Some("Agent")), &StillDesk, now);
    assert_eq!(cursor.motion().tip, Some(Point { x: 40.0, y: 30.0 }));
    assert!(cursor.motion().anchored);
    assert_eq!(cursor.label().opacity(ms(now, 180)), 1.0);
}

#[test]
fn a_travel_moves_by_elapsed_time_and_is_answered_on_arrival() {
    let now = Instant::now();
    let mut cursor = Cursor::new(look(), now, false);
    let path = vec![[0.0, 0.0], [10.0, 10.0], [20.0, 20.0]];
    assert!(
        cursor
            .travel(&styled(Some("Agent")), order(path, 7), now)
            .is_empty()
    );
    assert_eq!(
        cursor.label().opacity(ms(now, 5)),
        0.0,
        "the label waits for the arrival"
    );
    assert!(!cursor.motion().pointing, "the arrow shows on the way");
    assert_eq!(cursor.tick(ms(now, 12)), None);
    assert_eq!(cursor.motion().tip, Some(Point { x: 10.0, y: 10.0 }));
    assert_eq!(cursor.tick(ms(now, 20)), Some(7));
    assert_eq!(cursor.motion().tip, Some(Point { x: 20.0, y: 20.0 }));
    assert!(
        cursor.motion().pointing,
        "the pointer image shows on arrival"
    );
    assert_eq!(cursor.motion().moves, 1);
    assert!(cursor.label().opacity(ms(now, 200)) > 0.99);
}

#[test]
fn a_new_travel_answers_the_one_it_replaces() {
    let now = Instant::now();
    let mut cursor = Cursor::new(look(), now, false);
    let path = vec![[0.0, 0.0], [50.0, 50.0], [90.0, 90.0]];
    cursor.travel(&styled(None), order(path.clone(), 1), now);
    let answered = cursor.travel(&styled(None), order(path, 2), ms(now, 5));
    assert_eq!(answered, vec![1]);
}

#[test]
fn a_single_frame_travel_is_answered_at_once() {
    let now = Instant::now();
    let mut cursor = Cursor::new(look(), now, false);
    assert_eq!(
        cursor.travel(&styled(None), order(vec![[5.0, 6.0]], 3), now),
        vec![3]
    );
    assert_eq!(cursor.motion().tip, Some(Point { x: 5.0, y: 6.0 }));
}

#[test]
fn a_caret_follow_moves_only_a_shown_caret() {
    let now = Instant::now();
    let mut cursor = Cursor::new(look(), now, false);
    let rect = Rect {
        x: 1.0,
        y: 2.0,
        width: 2.0,
        height: 14.0,
    };
    cursor.caret_to(&styled(None), Some(rect), true, now);
    assert_eq!(cursor.caret(), None);
    cursor.caret_to(&styled(None), Some(rect), false, now);
    assert_eq!(cursor.caret(), Some(&rect));
    cursor.caret_to(&styled(None), None, true, now);
    assert_eq!(cursor.caret(), None);
}

#[test]
fn a_caret_request_keeps_the_label() {
    let now = Instant::now();
    let mut cursor = Cursor::new(look(), now, false);
    cursor.enable(&styled(Some("Agent")), &StillDesk, now);
    let rect = Rect {
        x: 1.0,
        y: 2.0,
        width: 2.0,
        height: 14.0,
    };
    cursor.caret_to(&styled(None), Some(rect), false, now);
    assert_eq!(cursor.label().text.as_deref(), Some("Agent"));
}

#[test]
fn a_finished_rest_clears_the_caret_and_a_follow_while_resting_changes_nothing() {
    let now = Instant::now();
    let mut cursor = Cursor::new(look(), now, false);
    let rect = Rect {
        x: 1.0,
        y: 2.0,
        width: 2.0,
        height: 14.0,
    };
    cursor.caret_to(&styled(None), Some(rect), false, now);
    let resting = now + IDLE_REST;
    cursor.caret_to(&styled(None), Some(rect), true, resting);
    assert_eq!(
        cursor.presence().woke,
        now,
        "a follow leaves a resting cursor resting"
    );
    let rested = now + IDLE_REST + REST_FADE;
    cursor.show(rested);
    assert_eq!(cursor.caret(), None);
}

#[test]
fn the_cursor_needs_frames_while_it_plays_and_sleeps_until_it_rests() {
    let now = Instant::now();
    let frame = Duration::from_millis(16);
    let mut cursor = Cursor::new(look(), now, false);
    assert_eq!(cursor.next_change(now, frame), Some(now + IDLE_REST));
    cursor.effect(
        &styled(None),
        (Point { x: 1.0, y: 1.0 }, None),
        Frames {
            frames: vec![[0.5, 0.0]; 3],
            frame_ms: 10.0,
        },
        now,
    );
    assert_eq!(cursor.next_change(now, frame), Some(now + frame));
    assert_eq!(
        cursor.next_change(ms(now, 25), frame),
        Some(now + IDLE_REST)
    );
    let grab = TravelOrder {
        drag: true,
        ..order(vec![[1.0, 1.0]], Ticket::default())
    };
    cursor.travel(&styled(None), grab, ms(now, 25));
    cursor.hide();
    assert_eq!(cursor.next_change(ms(now, 25), frame), None);
}
