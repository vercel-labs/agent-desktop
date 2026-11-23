//! The renderer's pixels, composited by hand into a premultiplied ARGB32
//! buffer: the layout a depth-32 TrueColor visual takes from `PutImage` on a
//! little-endian server. Every shape is filled with computed coverage, so its
//! edges stay smooth under a compositing manager.
use agent_desktop_core::Rect;

const MAX_SURFACE_PIXELS: usize = 16_777_216;
const SAMPLES: i32 = 3;

/// A premultiplied `A<<24 | R<<16 | G<<8 | B` surface, top-down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Surface {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) pixels: Vec<u32>,
}

/// A colour and the opacity it is painted at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Paint {
    pub(crate) rgb: [f64; 3],
    pub(crate) alpha: f64,
}

impl Surface {
    /// A fully transparent surface, or `None` when it would be empty or larger
    /// than any overlay element needs.
    pub(crate) fn transparent(width: u16, height: u16) -> Option<Self> {
        let count = usize::from(width).checked_mul(usize::from(height))?;
        if count == 0 || count > MAX_SURFACE_PIXELS {
            return None;
        }
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(count).ok()?;
        pixels.resize(count, 0);
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    pub(crate) fn pixel_at(&self, x: i32, y: i32) -> Option<u32> {
        let index = self.index(x, y)?;
        self.pixels.get(index).copied()
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let x = usize::try_from(x)
            .ok()
            .filter(|x| *x < usize::from(self.width))?;
        let y = usize::try_from(y)
            .ok()
            .filter(|y| *y < usize::from(self.height))?;
        Some(y * usize::from(self.width) + x)
    }

    /// Composites `paint` over one pixel at `coverage`, premultiplied
    /// source-over.
    pub(crate) fn blend(&mut self, x: i32, y: i32, paint: &Paint, coverage: f64) {
        let source_alpha = (paint.alpha * coverage).clamp(0.0, 1.0);
        if source_alpha <= 0.0 {
            return;
        }
        let Some(index) = self.index(x, y) else {
            return;
        };
        let Some(existing) = self.pixels.get(index).copied() else {
            return;
        };
        let under = |shift: u32| f64::from((existing >> shift) & 0xff) / 255.0;
        let byte = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
        let over = |shift: u32, source: f64| {
            byte(source * source_alpha + under(shift) * (1.0 - source_alpha))
        };
        let alpha = byte(source_alpha + under(24) * (1.0 - source_alpha));
        self.pixels[index] = (alpha << 24)
            | (over(16, paint.rgb[0]) << 16)
            | (over(8, paint.rgb[1]) << 8)
            | over(0, paint.rgb[2]);
    }

    /// Scales every pixel, all four premultiplied channels alike, by
    /// `opacity`, which fades the whole surface.
    pub(crate) fn dim(&mut self, opacity: f64) {
        let factor = opacity.clamp(0.0, 1.0);
        if factor >= 1.0 {
            return;
        }
        for pixel in &mut self.pixels {
            let scaled = |shift: u32| {
                ((f64::from((*pixel >> shift) & 0xff) * factor).round() as u32) << shift
            };
            *pixel = scaled(24) | scaled(16) | scaled(8) | scaled(0);
        }
    }

    /// The pixels as the bytes `PutImage` sends for a depth-32 image, in the
    /// server's image byte order.
    pub(crate) fn to_bytes(&self, most_significant_first: bool) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|pixel| {
                if most_significant_first {
                    pixel.to_be_bytes()
                } else {
                    pixel.to_le_bytes()
                }
            })
            .collect()
    }

    /// The same pixels for a server that cannot blend: every pixel at least
    /// half covered becomes opaque, every other one transparent, so the
    /// window's bounding shape can cut the transparent ones away.
    pub(crate) fn solidified(&self) -> Self {
        let pixels = self
            .pixels
            .iter()
            .map(|pixel| {
                let alpha = (pixel >> 24) & 0xff;
                if alpha < 128 {
                    return 0;
                }
                let channel = |shift: u32| {
                    let value = (pixel >> shift) & 0xff;
                    ((value * 255 + alpha / 2) / alpha).min(255) << shift
                };
                0xff00_0000 | channel(16) | channel(8) | channel(0)
            })
            .collect();
        Self {
            width: self.width,
            height: self.height,
            pixels,
        }
    }

    /// The opaque pixels as one rectangle per run within each row, the shape a
    /// window without a compositing manager is cut to.
    pub(crate) fn opaque_runs(&self) -> Vec<(i16, i16, u16)> {
        let mut runs = Vec::new();
        for y in 0..self.height {
            let mut start: Option<u16> = None;
            for x in 0..=self.width {
                let opaque = x < self.width
                    && self
                        .pixel_at(i32::from(x), i32::from(y))
                        .is_some_and(|pixel| pixel >> 24 >= 128);
                match (opaque, start) {
                    (true, None) => start = Some(x),
                    (false, Some(first)) => {
                        runs.push((first as i16, y as i16, x - first));
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        runs
    }
}

/// The share of pixel `(x, y)` that `inside` covers. A pixel whose corners
/// and centre agree is decided without sampling; an edge pixel is sampled on
/// a 3x3 grid.
fn coverage(x: i32, y: i32, inside: &impl Fn(f64, f64) -> bool) -> f64 {
    let left = f64::from(x);
    let top = f64::from(y);
    let first = inside(left, top);
    let undecided = [
        (left + 1.0, top),
        (left, top + 1.0),
        (left + 1.0, top + 1.0),
        (left + 0.5, top + 0.5),
    ]
    .iter()
    .any(|(px, py)| inside(*px, *py) != first);
    if !undecided {
        return if first { 1.0 } else { 0.0 };
    }
    let mut hits = 0;
    for sy in 0..SAMPLES {
        for sx in 0..SAMPLES {
            let px = left + (f64::from(sx) + 0.5) / f64::from(SAMPLES);
            let py = top + (f64::from(sy) + 0.5) / f64::from(SAMPLES);
            if inside(px, py) {
                hits += 1;
            }
        }
    }
    f64::from(hits) / f64::from(SAMPLES * SAMPLES)
}

fn pixel_span(surface: &Surface, bounds: &Rect) -> (i32, i32, i32, i32) {
    let x0 = bounds.x.floor().max(0.0) as i32;
    let y0 = bounds.y.floor().max(0.0) as i32;
    let x1 = (bounds.x + bounds.width)
        .ceil()
        .min(f64::from(surface.width)) as i32;
    let y1 = (bounds.y + bounds.height)
        .ceil()
        .min(f64::from(surface.height)) as i32;
    (x0, y0, x1, y1)
}

/// Fills the part of `bounds` that `inside` covers, skipping every pixel
/// whose whole cell lies in `skip`, where the caller knows `inside` is false.
pub(crate) fn fill_region(
    surface: &mut Surface,
    bounds: &Rect,
    skip: Option<&Rect>,
    paint: &Paint,
    inside: impl Fn(f64, f64) -> bool,
) {
    let (x0, y0, x1, y1) = pixel_span(surface, bounds);
    let skipped = |x: i32, y: i32| {
        skip.is_some_and(|skip| {
            f64::from(x) >= skip.x
                && f64::from(y) >= skip.y
                && f64::from(x + 1) <= skip.x + skip.width
                && f64::from(y + 1) <= skip.y + skip.height
        })
    };
    for y in y0..y1 {
        for x in x0..x1 {
            if !skipped(x, y) {
                let value = coverage(x, y, &inside);
                surface.blend(x, y, paint, value);
            }
        }
    }
}

/// Paints `paint` across `bounds` at the strength each pixel's centre gets,
/// for an edge that is a gradient rather than a shape.
pub(crate) fn fill_graded(
    surface: &mut Surface,
    bounds: &Rect,
    paint: &Paint,
    strength: impl Fn(f64, f64) -> f64,
) {
    let (x0, y0, x1, y1) = pixel_span(surface, bounds);
    for y in y0..y1 {
        for x in x0..x1 {
            let value = strength(f64::from(x) + 0.5, f64::from(y) + 0.5);
            surface.blend(x, y, paint, value);
        }
    }
}

pub(crate) fn point_in_polygon(points: &[(f64, f64)], x: f64, y: f64) -> bool {
    let Some(mut previous) = points.last().copied() else {
        return false;
    };
    let mut inside = false;
    for &(xi, yi) in points {
        let (xj, yj) = previous;
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        previous = (xi, yi);
    }
    inside
}

pub(crate) fn distance_to_polygon(points: &[(f64, f64)], x: f64, y: f64) -> f64 {
    let Some(mut previous) = points.last().copied() else {
        return f64::MAX;
    };
    let mut best = f64::MAX;
    for &point in points {
        best = best.min(distance_to_segment(previous, point, x, y));
        previous = point;
    }
    best
}

fn distance_to_segment(a: (f64, f64), b: (f64, f64), x: f64, y: f64) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (((x - a.0) * dx + (y - a.1) * dy) / length_squared).clamp(0.0, 1.0)
    };
    (x - (a.0 + t * dx)).hypot(y - (a.1 + t * dy))
}

/// The signed distance from a point to a rounded rectangle's edge: negative
/// inside, positive outside.
pub(crate) fn rounded_rect_distance(rect: &Rect, radius: f64, x: f64, y: f64) -> f64 {
    let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let dx = (x - (rect.x + rect.width / 2.0)).abs() - (rect.width / 2.0 - radius);
    let dy = (y - (rect.y + rect.height / 2.0)).abs() - (rect.height / 2.0 - radius);
    dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - radius
}

pub(crate) fn in_rounded_rect(rect: &Rect, radius: f64, x: f64, y: f64) -> bool {
    rect.width > 0.0 && rect.height > 0.0 && rounded_rect_distance(rect, radius, x, y) <= 0.0
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;
