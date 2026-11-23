use agent_desktop_core::Rect;

use super::*;

const WHITE: Paint = Paint {
    rgb: [1.0, 1.0, 1.0],
    alpha: 1.0,
};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn an_empty_or_oversized_surface_is_refused() {
    assert!(Surface::transparent(0, 10).is_none());
    assert!(Surface::transparent(u16::MAX, u16::MAX).is_none());
    let surface = Surface::transparent(3, 2).expect("small surface");
    assert_eq!(surface.pixels, vec![0; 6]);
}

#[test]
fn blending_is_premultiplied_source_over() {
    let mut surface = Surface::transparent(1, 1).expect("surface");
    surface.blend(
        0,
        0,
        &Paint {
            rgb: [1.0, 0.0, 0.0],
            alpha: 0.5,
        },
        1.0,
    );
    assert_eq!(surface.pixel_at(0, 0), Some(0x8080_0000));
    surface.blend(
        0,
        0,
        &Paint {
            rgb: [0.0, 0.0, 1.0],
            alpha: 1.0,
        },
        0.5,
    );
    assert_eq!(surface.pixel_at(0, 0), Some(0xc040_0080));
}

#[test]
fn blending_outside_the_surface_or_at_no_coverage_changes_nothing() {
    let mut surface = Surface::transparent(2, 2).expect("surface");
    surface.blend(-1, 0, &WHITE, 1.0);
    surface.blend(2, 0, &WHITE, 1.0);
    surface.blend(0, 0, &WHITE, 0.0);
    assert_eq!(surface.pixels, vec![0; 4]);
}

#[test]
fn dimming_scales_every_channel() {
    let mut surface = Surface::transparent(1, 1).expect("surface");
    surface.blend(0, 0, &WHITE, 1.0);
    surface.dim(0.5);
    assert_eq!(surface.pixel_at(0, 0), Some(0x8080_8080));
    surface.dim(0.0);
    assert_eq!(surface.pixel_at(0, 0), Some(0));
}

#[test]
fn the_bytes_follow_the_server_byte_order() {
    let mut surface = Surface::transparent(1, 1).expect("surface");
    surface.pixels[0] = 0xff11_2233;
    assert_eq!(surface.to_bytes(false), vec![0x33, 0x22, 0x11, 0xff]);
    assert_eq!(surface.to_bytes(true), vec![0xff, 0x11, 0x22, 0x33]);
}

#[test]
fn solidifying_keeps_half_covered_pixels_opaque_and_drops_the_rest() {
    let mut surface = Surface::transparent(2, 1).expect("surface");
    surface.pixels = vec![0x8040_0000, 0x7f7f_7f7f];
    let solid = surface.solidified();
    assert_eq!(solid.pixels, vec![0xff80_0000, 0]);
}

#[test]
fn opaque_runs_cover_each_row_of_opaque_pixels() {
    let mut surface = Surface::transparent(4, 2).expect("surface");
    surface.pixels = vec![
        0xff00_0000,
        0xff00_0000,
        0,
        0xff00_0000,
        0,
        0,
        0,
        0x8000_0000,
    ];
    assert_eq!(surface.opaque_runs(), vec![(0, 0, 2), (3, 0, 1), (3, 1, 1)]);
}

#[test]
fn a_filled_half_plane_has_full_inner_pixels_and_partial_edge_pixels() {
    let mut surface = Surface::transparent(4, 1).expect("surface");
    let bounds = rect(0.0, 0.0, 4.0, 1.0);
    fill_region(&mut surface, &bounds, None, &WHITE, |x, _| x <= 1.5);
    let alpha = |x: i32| surface.pixel_at(x, 0).map_or(0, |pixel| pixel >> 24);
    assert_eq!(alpha(0), 255);
    assert!((100..=200).contains(&alpha(1)), "{}", alpha(1));
    assert_eq!(alpha(2), 0);
}

#[test]
fn a_skipped_cell_is_left_unpainted() {
    let mut surface = Surface::transparent(3, 3).expect("surface");
    let bounds = rect(0.0, 0.0, 3.0, 3.0);
    let skip = rect(1.0, 1.0, 1.0, 1.0);
    fill_region(&mut surface, &bounds, Some(&skip), &WHITE, |_, _| true);
    assert_eq!(surface.pixel_at(1, 1), Some(0));
    assert_eq!(surface.pixel_at(0, 0), Some(0xffff_ffff));
}

#[test]
fn graded_fill_uses_each_pixel_centre() {
    let mut surface = Surface::transparent(2, 1).expect("surface");
    fill_graded(&mut surface, &rect(0.0, 0.0, 2.0, 1.0), &WHITE, |x, _| {
        if x < 1.0 { 1.0 } else { 0.0 }
    });
    assert_eq!(surface.pixel_at(0, 0), Some(0xffff_ffff));
    assert_eq!(surface.pixel_at(1, 0), Some(0));
}

#[test]
fn polygon_helpers_answer_inside_and_distance() {
    let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
    assert!(point_in_polygon(&square, 5.0, 5.0));
    assert!(!point_in_polygon(&square, 15.0, 5.0));
    assert!(!point_in_polygon(&[], 0.0, 0.0));
    assert!((distance_to_polygon(&square, 15.0, 5.0) - 5.0).abs() < 1e-9);
}

#[test]
fn a_rounded_rectangle_excludes_its_corners() {
    let pill = rect(0.0, 0.0, 40.0, 20.0);
    assert!(in_rounded_rect(&pill, 10.0, 20.0, 10.0));
    assert!(!in_rounded_rect(&pill, 10.0, 0.5, 0.5));
    assert!(in_rounded_rect(&pill, 10.0, 10.0, 0.5));
    assert!(!in_rounded_rect(&rect(0.0, 0.0, 0.0, 5.0), 1.0, 0.0, 0.0));
    assert!((rounded_rect_distance(&pill, 10.0, 50.0, 10.0) - 10.0).abs() < 1e-9);
}
