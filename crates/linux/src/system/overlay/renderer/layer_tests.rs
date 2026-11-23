use std::time::{Duration, Instant};

use agent_desktop_core::Rect;

use super::*;
use crate::system::overlay::renderer::request::{Frames, Look};

struct TwoMonitors;

impl Desk for TwoMonitors {
    fn pointer(&self) -> Point {
        Point { x: 0.0, y: 0.0 }
    }

    fn monitor_at(&self, at: &Point) -> Rect {
        if at.x >= 1000.0 {
            Rect {
                x: 1000.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            }
        } else {
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 700.0,
            }
        }
    }

    fn work_area_at(&self, at: &Point) -> Rect {
        self.monitor_at(at)
    }
}

fn owner(agent: Option<&str>) -> Owner {
    Owner {
        session: "s1".into(),
        agent: agent.map(str::to_owned),
    }
}

fn styled(agent: Option<&str>) -> Styled {
    Styled {
        owner: owner(agent),
        look: Look {
            fill: [0.0; 3],
            rim: [0.0; 3],
            accent: [0.0; 3],
            size: 1.0,
            arrow: None,
            pointer: None,
        },
        label: None,
    }
}

fn moving(agent: Option<&str>, frames: Vec<[f64; 2]>, drag: bool) -> Request {
    Request::Travel {
        styled: styled(agent),
        path: Frames {
            frames,
            frame_ms: 10.0,
        },
        pointer: false,
        clear_caret: false,
        drag,
    }
}

fn travel(agent: Option<&str>, frames: Vec<[f64; 2]>) -> Request {
    moving(agent, frames, false)
}

#[test]
fn an_unknown_cursor_sets_off_below_left_of_the_destination_on_its_monitor() {
    let layer = CursorLayer::default();
    let start = layer.locate(
        &LocateRequest {
            owner: owner(None),
            destination: Point {
                x: 1100.0,
                y: 550.0,
            },
        },
        &TwoMonitors,
    );
    assert_eq!(
        start,
        CursorStart {
            x: 1000.0,
            y: 600.0,
            moves: 0
        }
    );
}

#[test]
fn an_anchored_cursor_sets_off_from_its_tip_through_a_headed_hide_but_not_after_a_cancelled_drag() {
    let now = Instant::now();
    let mut layer = CursorLayer::default();
    layer.update(travel(None, vec![[300.0, 200.0]]), 1, now, &TwoMonitors);
    let request = LocateRequest {
        owner: owner(None),
        destination: Point { x: 500.0, y: 100.0 },
    };
    assert_eq!(
        layer.locate(&request, &TwoMonitors),
        CursorStart {
            x: 300.0,
            y: 200.0,
            moves: 1
        }
    );
    layer.update(Request::Hide(owner(None)), 2, now, &TwoMonitors);
    assert!(
        layer
            .cursors()
            .all(|(_, cursor)| cursor.presence().is_shown())
    );
    assert_eq!(
        layer.locate(&request, &TwoMonitors),
        CursorStart {
            x: 300.0,
            y: 200.0,
            moves: 1
        }
    );
    layer.update(
        moving(None, vec![[400.0, 150.0]], true),
        3,
        now,
        &TwoMonitors,
    );
    layer.update(Request::Hide(owner(None)), 4, now, &TwoMonitors);
    assert!(layer.cursors().all(|(_, cursor)| cursor.presence().hidden));
    assert_eq!(
        layer.locate(&request, &TwoMonitors),
        CursorStart {
            x: 320.0,
            y: 208.0,
            moves: 2
        }
    );
}

#[test]
fn a_drag_that_ends_in_its_drop_effect_keeps_its_anchor_through_a_hide() {
    let now = Instant::now();
    let mut layer = CursorLayer::default();
    layer.update(
        moving(None, vec![[400.0, 150.0]], true),
        1,
        now,
        &TwoMonitors,
    );
    layer.update(
        Request::Effect {
            styled: styled(None),
            point: Point { x: 700.0, y: 450.0 },
            target: None,
            path: Frames {
                frames: Vec::new(),
                frame_ms: 10.0,
            },
        },
        2,
        now,
        &TwoMonitors,
    );
    layer.update(Request::Hide(owner(None)), 3, now, &TwoMonitors);
    assert!(
        layer
            .cursors()
            .all(|(_, cursor)| cursor.presence().is_shown())
    );
    let start = layer.locate(
        &LocateRequest {
            owner: owner(None),
            destination: Point { x: 900.0, y: 500.0 },
        },
        &TwoMonitors,
    );
    assert_eq!((start.x, start.y), (700.0, 450.0));
}

#[test]
fn a_travel_is_answered_when_it_arrives_and_others_at_once() {
    let now = Instant::now();
    let mut layer = CursorLayer::default();
    assert!(
        layer
            .update(
                travel(None, vec![[0.0, 0.0], [5.0, 5.0]]),
                1,
                now,
                &TwoMonitors
            )
            .is_empty()
    );
    assert_eq!(
        layer.update(Request::Enable(styled(Some("b"))), 2, now, &TwoMonitors),
        vec![2]
    );
    assert!(layer.tick(now + Duration::from_millis(5)).is_empty());
    assert_eq!(layer.tick(now + Duration::from_millis(10)), vec![1]);
}

#[test]
fn disabling_a_session_answers_its_travels_and_removes_its_cursors() {
    let now = Instant::now();
    let mut layer = CursorLayer::default();
    layer.update(
        travel(Some("a"), vec![[0.0, 0.0], [5.0, 5.0]]),
        1,
        now,
        &TwoMonitors,
    );
    let answered = layer.update(Request::Disable("s1".into()), 2, now, &TwoMonitors);
    assert_eq!(answered, vec![2, 1]);
    assert!(layer.is_empty());
}

#[test]
fn hide_without_an_agent_reaches_every_cursor_of_the_session() {
    let now = Instant::now();
    let mut layer = CursorLayer::default();
    layer.update(
        moving(Some("a"), vec![[10.0, 10.0]], true),
        1,
        now,
        &TwoMonitors,
    );
    layer.update(
        moving(Some("b"), vec![[20.0, 20.0]], true),
        2,
        now,
        &TwoMonitors,
    );
    layer.update(Request::Hide(owner(Some("a"))), 3, now, &TwoMonitors);
    let hidden: Vec<bool> = layer
        .cursors()
        .map(|(_, cursor)| cursor.presence().hidden)
        .collect();
    assert_eq!(hidden, vec![true, false]);
    layer.update(Request::Hide(owner(None)), 4, now, &TwoMonitors);
    assert!(layer.cursors().all(|(_, cursor)| cursor.presence().hidden));
}

#[test]
fn overlapping_captures_keep_the_cursors_hidden_until_the_last_reveal() {
    let now = Instant::now();
    let mut layer = CursorLayer::default();
    layer.update(Request::Enable(styled(None)), 1, now, &TwoMonitors);
    layer.update(Request::Conceal, 2, now, &TwoMonitors);
    layer.update(Request::Conceal, 3, now, &TwoMonitors);
    layer.update(Request::Enable(styled(Some("late"))), 4, now, &TwoMonitors);
    layer.update(Request::Reveal, 5, now, &TwoMonitors);
    assert!(
        layer
            .cursors()
            .all(|(_, cursor)| cursor.presence().concealed)
    );
    layer.update(Request::Reveal, 6, now, &TwoMonitors);
    assert!(
        layer
            .cursors()
            .all(|(_, cursor)| !cursor.presence().concealed)
    );
}

#[test]
fn a_conceal_no_one_reveals_is_lifted_after_the_limit() {
    let now = Instant::now();
    let mut layer = CursorLayer::default();
    layer.update(Request::Enable(styled(None)), 1, now, &TwoMonitors);
    layer.update(Request::Conceal, 2, now, &TwoMonitors);
    layer.tick(now + CONCEAL_LIMIT);
    assert!(
        layer
            .cursors()
            .all(|(_, cursor)| !cursor.presence().concealed)
    );
    layer.update(Request::Reveal, 3, now, &TwoMonitors);
    layer.update(Request::Conceal, 4, now, &TwoMonitors);
    layer.update(Request::Reveal, 5, now, &TwoMonitors);
    assert!(
        layer
            .cursors()
            .all(|(_, cursor)| !cursor.presence().concealed)
    );
}

#[test]
fn a_caret_follow_for_an_unknown_cursor_makes_no_cursor() {
    let now = Instant::now();
    let mut layer = CursorLayer::default();
    let follow = Request::Caret {
        styled: styled(None),
        rect: None,
        follow: true,
    };
    assert_eq!(layer.update(follow, 1, now, &TwoMonitors), vec![1]);
    assert!(layer.is_empty());
}
