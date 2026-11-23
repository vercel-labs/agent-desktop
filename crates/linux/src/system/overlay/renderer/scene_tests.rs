use std::time::{Duration, Instant};

use agent_desktop_core::Rect;

use super::*;
use crate::system::overlay::renderer::presence::{IDLE_REST, REST_FADE};
use crate::system::overlay::renderer::request::{Frames, Look, Owner, Styled};
use crate::system::overlay::renderer::travel::Ticket;
use crate::system::overlay::renderer::travel_order::TravelOrder;

struct Screen;

impl Desk for Screen {
    fn pointer(&self) -> Point {
        Point { x: 0.0, y: 0.0 }
    }

    fn monitor_at(&self, _at: &Point) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        }
    }

    fn work_area_at(&self, _at: &Point) -> Rect {
        Rect {
            x: 0.0,
            y: 32.0,
            width: 800.0,
            height: 568.0,
        }
    }
}

fn styled(label: Option<&str>) -> Styled {
    Styled {
        owner: Owner {
            session: "s1".into(),
            agent: None,
        },
        look: Look {
            fill: [0.2, 0.6, 1.0],
            rim: [0.0; 3],
            accent: [1.0, 0.5, 0.0],
            size: 1.0,
            arrow: None,
            pointer: None,
        },
        label: label.map(str::to_owned),
    }
}

fn placed_at(x: f64, y: f64, label: Option<&str>, now: Instant) -> Cursor {
    let mut cursor = Cursor::new(styled(label).look, now, false);
    let order = TravelOrder {
        path: Frames {
            frames: vec![[x, y]],
            frame_ms: 10.0,
        },
        pointer: false,
        clear_caret: false,
        drag: false,
        ticket: Ticket::default(),
    };
    cursor.travel(&styled(label), order, now);
    cursor
}

fn slots(pieces: &[Piece]) -> Vec<Slot> {
    pieces.iter().map(|piece| piece.slot).collect()
}

#[test]
fn the_dart_is_drawn_with_its_tip_on_the_cursor_point() {
    let now = Instant::now();
    let cursor = placed_at(100.0, 200.0, None, now);
    let pieces = compose(
        &cursor,
        &Scenery {
            now,
            desk: &Screen,
            font: None,
        },
    );
    assert_eq!(slots(&pieces), vec![Slot::Glyph]);
    assert_eq!(
        pieces[0].origin,
        (93, 189),
        "the tip sits 7.5 by 11.5 into the dart"
    );
    assert_eq!(pieces[0].opacity, 1.0);
}

#[test]
fn the_pill_goes_right_of_and_below_the_tip_inside_the_work_area() {
    let font =
        LabelFont::load().expect("a sans-serif font: install fontconfig and fonts-dejavu-core");
    let now = Instant::now();
    let cursor = placed_at(100.0, 200.0, Some("Agent"), now);
    let later = now + Duration::from_secs(1);
    let pieces = compose(
        &cursor,
        &Scenery {
            now: later,
            desk: &Screen,
            font: Some(&font),
        },
    );
    assert_eq!(slots(&pieces), vec![Slot::Pill, Slot::Glyph]);
    let pill = &pieces[0];
    assert_eq!(
        pill.origin,
        (120 - 5, 230 - 5),
        "offset 20 by 30, less the shadow margin"
    );
    assert_eq!(pill.opacity, 1.0);
    let Art::Pill { height, text, .. } = &pill.art else {
        panic!("a pill: {:?}", pill.art);
    };
    assert_eq!((*height, text.as_str()), (24.0, "Agent"));
    let near_right = placed_at(790.0, 10.0, Some("Agent"), now);
    let pieces = compose(
        &near_right,
        &Scenery {
            now: later,
            desk: &Screen,
            font: Some(&font),
        },
    );
    let (x, y) = pieces[0].origin;
    assert!(x + 5 < 790, "mirrored left of the tip: {x}");
    assert_eq!(y + 5, 40, "below the tip but not above the work area");
}

#[test]
fn a_click_shows_the_ripple_over_the_tip_and_the_outline_around_the_target() {
    let now = Instant::now();
    let mut cursor = placed_at(0.0, 0.0, None, now);
    let target = Rect {
        x: 50.0,
        y: 60.0,
        width: 100.0,
        height: 20.0,
    };
    let path = Frames {
        frames: vec![[0.25, 0.5], [0.5, 1.0], [0.75, 0.2]],
        frame_ms: 10.0,
    };
    cursor.effect(
        &styled(None),
        (Point { x: 100.0, y: 70.0 }, Some(target)),
        path,
        now,
    );
    let pieces = compose(
        &cursor,
        &Scenery {
            now: now + Duration::from_millis(12),
            desk: &Screen,
            font: None,
        },
    );
    assert_eq!(
        slots(&pieces),
        vec![Slot::Highlight, Slot::Ripple, Slot::Glyph]
    );
    assert_eq!(pieces[0].origin, (45, 55));
    assert_eq!(pieces[0].opacity, 1.0);
    assert_eq!(pieces[1].origin, (46, 16));
    let done = compose(
        &cursor,
        &Scenery {
            now: now + Duration::from_millis(20),
            desk: &Screen,
            font: None,
        },
    );
    assert_eq!(
        slots(&done),
        vec![Slot::Glyph],
        "the effect stops once it has played"
    );
}

#[test]
fn a_hidden_or_rested_cursor_shows_nothing_and_a_resting_one_fades() {
    let now = Instant::now();
    let mut cursor = placed_at(10.0, 10.0, None, now);
    let fading = compose(
        &cursor,
        &Scenery {
            now: now + IDLE_REST + REST_FADE / 2,
            desk: &Screen,
            font: None,
        },
    );
    assert!(
        (fading[0].opacity - 0.5).abs() < 0.02,
        "{}",
        fading[0].opacity
    );
    assert!(
        compose(
            &cursor,
            &Scenery {
                now: now + IDLE_REST + REST_FADE,
                desk: &Screen,
                font: None
            }
        )
        .is_empty()
    );
    let grab = TravelOrder {
        path: Frames {
            frames: vec![[100.0, 100.0]],
            frame_ms: 10.0,
        },
        pointer: false,
        clear_caret: false,
        drag: true,
        ticket: Ticket::default(),
    };
    cursor.travel(&styled(None), grab, now);
    cursor.hide();
    assert!(
        compose(
            &cursor,
            &Scenery {
                now,
                desk: &Screen,
                font: None
            }
        )
        .is_empty()
    );
}

#[test]
fn the_caret_is_a_bar_at_least_one_pixel_wide() {
    let now = Instant::now();
    let mut cursor = Cursor::new(styled(None).look, now, false);
    cursor.caret_to(
        &styled(None),
        Some(Rect {
            x: 10.4,
            y: 20.6,
            width: 0.2,
            height: 15.0,
        }),
        false,
        now,
    );
    let pieces = compose(
        &cursor,
        &Scenery {
            now,
            desk: &Screen,
            font: None,
        },
    );
    assert_eq!(
        slots(&pieces),
        vec![Slot::Caret],
        "an unplaced cursor shows only its caret"
    );
    assert_eq!(pieces[0].origin, (10, 21));
    assert_eq!(
        pieces[0].art,
        Art::Caret {
            width: 1.0,
            height: 15.0,
            accent: [1.0, 0.5, 0.0]
        }
    );
}
