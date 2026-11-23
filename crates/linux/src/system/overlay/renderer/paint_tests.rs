use super::*;

const FILL: [f64; 3] = [0.26, 0.6, 1.0];
const RIM: [f64; 3] = [0.07, 0.07, 0.09];

fn alpha(surface: &Surface, x: i32, y: i32) -> u32 {
    surface.pixel_at(x, y).map_or(0, |pixel| pixel >> 24)
}

#[test]
fn the_dart_has_its_tip_rimmed_and_its_body_filled() {
    let surface = dart(1.0, FILL, RIM).expect("dart");
    assert_eq!((surface.width, surface.height), (45, 53));
    let (tip_x, tip_y) = dart_tip(1.0);
    assert_eq!((tip_x, tip_y), (7.5, 11.5));
    assert_eq!(alpha(&surface, 0, 0), 0);
    let body = surface.pixel_at(16, 30).expect("inside the dart");
    assert_eq!(body >> 24, 255);
    assert_eq!(body & 0xff, 255, "the body is the fill colour");
    let rim = surface
        .pixel_at(tip_x as i32, tip_y as i32 - 2)
        .expect("rim");
    assert_eq!(rim >> 24, 255);
    assert!(rim & 0xff < 40, "the edge is the rim colour: {rim:08x}");
}

#[test]
fn the_dart_scales_with_the_style_size() {
    let surface = dart(2.0, FILL, RIM).expect("dart");
    assert_eq!((surface.width, surface.height), (90, 106));
    assert_eq!(dart_tip(2.0), (15.0, 23.0));
}

#[test]
fn the_ripple_is_drawn_only_inside_its_phase() {
    assert!(ripple(1.0, 0.0, FILL).is_none());
    assert!(ripple(1.0, 1.5, FILL).is_none());
    let early = ripple(1.0, 0.1, FILL).expect("early ripple");
    assert_eq!((early.width, early.height), (108, 108));
    assert!(alpha(&early, 54, 54) > 120, "the core is solid early");
    let late = ripple(1.0, 0.9, FILL).expect("late ripple");
    assert!(alpha(&late, 54, 54) < alpha(&early, 54, 54));
    let ring = (54.0 - 4.0) * 0.9;
    assert!(
        alpha(&late, 54 + ring as i32 - 1, 54) > 0,
        "the ring has spread"
    );
}

#[test]
fn the_highlight_is_a_hollow_ring_around_the_padded_target() {
    let surface = highlight(1.0, 100.0, 40.0, FILL).expect("highlight");
    assert_eq!((surface.width, surface.height), (110, 50));
    assert_eq!(alpha(&surface, 55, 1), 255, "the top edge is drawn");
    assert_eq!(alpha(&surface, 55, 25), 0, "the inside stays clear");
    assert_eq!(alpha(&surface, 0, 0), 0, "the corner is rounded off");
}

#[test]
fn the_pill_has_an_opaque_body_and_a_soft_shadow_below() {
    let (side, below) = shadow_margins(1.0);
    assert_eq!((side, below), (5.0, 7.0));
    let surface = pill(1.0, 80.0, 24.0).expect("pill");
    assert_eq!((surface.width, surface.height), (90, 36));
    assert_eq!(surface.pixel_at(45, 17), Some(0xff00_0000));
    let shadow = alpha(&surface, 45, 31);
    assert!(shadow > 0 && shadow < 255, "shadow alpha {shadow}");
    assert_eq!(alpha(&surface, 0, 0), 0);
}

#[test]
fn the_caret_is_a_solid_bar() {
    let surface = caret(2.0, 10.0, FILL).expect("caret");
    assert_eq!((surface.width, surface.height), (2, 10));
    assert!(surface.pixels.iter().all(|pixel| pixel >> 24 == 255));
    assert!(caret(0.0, 10.0, FILL).is_none());
}
