//! The overlay's shapes, each painted on its own surface: the dart, the click
//! ripple, the outline around the acted-on element, the label pill and the
//! caret. Sizes are the GNOME Shell extension's, scaled by the style's size,
//! so every Linux overlay draws the same cursor.
use agent_desktop_core::{LABEL_SHADOW_BLUR, LABEL_SHADOW_OFFSET, LABEL_SHADOW_OPACITY, Rect};

use super::raster::{self, Paint, Surface};

pub(crate) const DART: [(f64, f64); 4] = [(1.0, 5.0), (29.6, 22.5), (12.7, 23.7), (4.2, 38.4)];
pub(crate) const GLYPH_WIDTH: f64 = 32.0;
pub(crate) const GLYPH_HEIGHT: f64 = 40.0;
pub(crate) const GLYPH_RIM_WIDTH: f64 = 6.5;
pub(crate) const RIPPLE_SIZE: f64 = 108.0;
const RIPPLE_CORE_RADIUS: f64 = 19.0;
const RIPPLE_RING_INSET: f64 = 4.0;
const RIPPLE_RING_WIDTH: f64 = 3.0;
pub(crate) const HIGHLIGHT_PADDING: f64 = 5.0;
const HIGHLIGHT_BORDER_WIDTH: f64 = 2.5;
const HIGHLIGHT_CORNER_RADIUS: f64 = 8.0;

fn dimension(length: f64) -> Option<u16> {
    let length = length.ceil();
    (length.is_finite() && length >= 1.0 && length <= f64::from(u16::MAX)).then_some(length as u16)
}

fn solid(rgb: [f64; 3], alpha: f64) -> Paint {
    Paint { rgb, alpha }
}

/// Where the dart's tip sits on its surface, which is where the surface is
/// placed relative to the cursor's tip.
pub(crate) fn dart_tip(scale: f64) -> (f64, f64) {
    let rim = GLYPH_RIM_WIDTH * scale;
    (DART[0].0 * scale + rim, DART[0].1 * scale + rim)
}

/// The rimmed dart: the rim is the outline stroked half outside the shape,
/// with round joins, and the fill is the shape itself.
pub(crate) fn dart(scale: f64, fill: [f64; 3], rim: [f64; 3]) -> Option<Surface> {
    let margin = GLYPH_RIM_WIDTH * scale;
    let mut surface = Surface::transparent(
        dimension(GLYPH_WIDTH * scale + 2.0 * margin)?,
        dimension(GLYPH_HEIGHT * scale + 2.0 * margin)?,
    )?;
    let points: Vec<(f64, f64)> = DART
        .iter()
        .map(|(x, y)| (margin + x * scale, margin + y * scale))
        .collect();
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: f64::from(surface.width),
        height: f64::from(surface.height),
    };
    let half = margin / 2.0;
    raster::fill_region(&mut surface, &bounds, None, &solid(rim, 1.0), |x, y| {
        raster::point_in_polygon(&points, x, y)
            || raster::distance_to_polygon(&points, x, y) <= half
    });
    raster::fill_region(&mut surface, &bounds, None, &solid(fill, 1.0), |x, y| {
        raster::point_in_polygon(&points, x, y)
    });
    Some(surface)
}

/// The side of the ripple's square surface, centred on the tip.
pub(crate) fn ripple_side(scale: f64) -> f64 {
    RIPPLE_SIZE * scale
}

/// The click ripple at `phase`: a shrinking core and a widening ring, both
/// fading as the phase runs from 0 to 1. `None` outside that range.
pub(crate) fn ripple(scale: f64, phase: f64, accent: [f64; 3]) -> Option<Surface> {
    if phase <= 0.0 || phase > 1.0 {
        return None;
    }
    let side = dimension(ripple_side(scale))?;
    let mut surface = Surface::transparent(side, side)?;
    let centre = ripple_side(scale) / 2.0;
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: f64::from(side),
        height: f64::from(side),
    };
    let fade = 1.0 - phase;
    let core = RIPPLE_CORE_RADIUS * scale * (1.0 - phase * 0.4);
    let distance = move |x: f64, y: f64| (x - centre).hypot(y - centre);
    raster::fill_region(
        &mut surface,
        &bounds,
        None,
        &solid(accent, 0.55 * fade),
        |x, y| distance(x, y) <= core,
    );
    let ring = (RIPPLE_SIZE / 2.0 - RIPPLE_RING_INSET) * scale * phase;
    let width = (RIPPLE_RING_WIDTH * scale).min(ring);
    if width > 0.0 {
        raster::fill_region(
            &mut surface,
            &bounds,
            None,
            &solid(accent, 0.85 * fade),
            |x, y| (ring - width..=ring).contains(&distance(x, y)),
        );
    }
    Some(surface)
}

/// The outline around a target `width` by `height`, padded on every side: a
/// rounded ring in the accent colour. Its surface starts the padding above and
/// left of the target.
pub(crate) fn highlight(scale: f64, width: f64, height: f64, accent: [f64; 3]) -> Option<Surface> {
    let padding = HIGHLIGHT_PADDING * scale;
    let mut surface = Surface::transparent(
        dimension(width + padding * 2.0)?,
        dimension(height + padding * 2.0)?,
    )?;
    let outer = Rect {
        x: 0.0,
        y: 0.0,
        width: width + padding * 2.0,
        height: height + padding * 2.0,
    };
    let border = HIGHLIGHT_BORDER_WIDTH * scale;
    let radius = HIGHLIGHT_CORNER_RADIUS * scale;
    let inner = Rect {
        x: border,
        y: border,
        width: (outer.width - border * 2.0).max(0.0),
        height: (outer.height - border * 2.0).max(0.0),
    };
    let interior = Rect {
        x: border * 2.0,
        y: border + radius,
        width: outer.width - border * 4.0,
        height: outer.height - (border + radius) * 2.0,
    };
    raster::fill_region(
        &mut surface,
        &outer,
        Some(&interior),
        &solid(accent, 1.0),
        |x, y| {
            raster::in_rounded_rect(&outer, radius, x, y)
                && !raster::in_rounded_rect(&inner, (radius - border).max(0.0), x, y)
        },
    );
    Some(surface)
}

/// How far the pill's soft shadow reaches past the pill: above and to each
/// side, and below, where it is also offset down.
pub(crate) fn shadow_margins(scale: f64) -> (f64, f64) {
    let blur = (LABEL_SHADOW_BLUR * scale).max(0.5);
    let reach = (blur * 2.0).ceil();
    (reach, reach + (LABEL_SHADOW_OFFSET * scale).ceil())
}

/// The label pill `width` by `height` on a surface with room for its shadow:
/// a soft black shadow, then an opaque black, fully rounded body. The body's
/// top-left corner sits at the side margin of [`shadow_margins`].
pub(crate) fn pill(scale: f64, width: f64, height: f64) -> Option<Surface> {
    let (side, below) = shadow_margins(scale);
    let mut surface = Surface::transparent(
        dimension(width + side * 2.0)?,
        dimension(height + side + below)?,
    )?;
    let body = Rect {
        x: side,
        y: side,
        width,
        height,
    };
    let radius = height / 2.0;
    let blur = (LABEL_SHADOW_BLUR * scale).max(0.5);
    let shadow = Rect {
        y: body.y + LABEL_SHADOW_OFFSET * scale,
        ..body
    };
    let everything = Rect {
        x: 0.0,
        y: 0.0,
        width: f64::from(surface.width),
        height: f64::from(surface.height),
    };
    raster::fill_graded(
        &mut surface,
        &everything,
        &solid([0.0; 3], LABEL_SHADOW_OPACITY),
        |x, y| shadow_strength(raster::rounded_rect_distance(&shadow, radius, x, y), blur),
    );
    raster::fill_region(&mut surface, &body, None, &solid([0.0; 3], 1.0), |x, y| {
        raster::in_rounded_rect(&body, radius, x, y)
    });
    Some(surface)
}

/// How strongly a shadow covers a point `distance` outside its shape
/// (negative inside): full well inside, none a blur's width outside, easing
/// between.
fn shadow_strength(distance: f64, blur: f64) -> f64 {
    let t = ((distance + blur) / (blur * 2.0)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// The typing caret: a solid accent bar.
pub(crate) fn caret(width: f64, height: f64, accent: [f64; 3]) -> Option<Surface> {
    let mut surface = Surface::transparent(dimension(width)?, dimension(height)?)?;
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: f64::from(surface.width),
        height: f64::from(surface.height),
    };
    raster::fill_region(&mut surface, &bounds, None, &solid(accent, 1.0), |_, _| {
        true
    });
    Some(surface)
}

#[cfg(test)]
#[path = "paint_tests.rs"]
mod tests;
