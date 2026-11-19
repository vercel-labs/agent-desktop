//! The label's text, laid out and rasterized with `ab_glyph` in white on the
//! pill. The font is the desktop's semibold sans-serif as fontconfig names it,
//! with a few common files as fallbacks; without any, the label is not drawn.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ab_glyph::{Font, FontVec, GlyphId, PxScale, ScaleFont, VariableFont, point};
use agent_desktop_core::Rect;

use super::raster::{Paint, Surface};

const MAX_FONT_BYTES: u64 = 32 * 1024 * 1024;
const SEMIBOLD: f32 = 600.0;
const ELLIPSIS: char = '\u{2026}';
const FALLBACKS: [&str; 4] = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
];
const WHITE: Paint = Paint {
    rgb: [1.0, 1.0, 1.0],
    alpha: 1.0,
};

/// The font the label is set in.
pub(crate) struct LabelFont {
    font: FontVec,
}

fn fontconfig_choice() -> Option<PathBuf> {
    let output = Command::new("fc-match")
        .args(["--format=%{file}", "sans-serif:weight=semibold"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    (output.status.success() && path.is_absolute()).then_some(path)
}

fn read_font(path: &Path) -> Option<FontVec> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_FONT_BYTES {
        return None;
    }
    let mut font = FontVec::try_from_vec(std::fs::read(path).ok()?).ok()?;
    font.set_variation(b"wght", SEMIBOLD);
    Some(font)
}

impl LabelFont {
    /// The desktop's sans-serif, or the first readable fallback.
    pub(crate) fn load() -> Option<Self> {
        fontconfig_choice()
            .into_iter()
            .chain(FALLBACKS.iter().map(PathBuf::from))
            .find_map(|path| read_font(&path))
            .map(|font| Self { font })
    }

    fn scale(&self, em: f64) -> Option<PxScale> {
        let units = self.font.units_per_em()?;
        let height = self.font.height_unscaled();
        let scale = em as f32 * height / units;
        (scale.is_finite() && scale > 0.0).then(|| PxScale::from(scale))
    }

    fn glyphs(&self, text: &str) -> Vec<GlyphId> {
        text.chars().map(|c| self.font.glyph_id(c)).collect()
    }

    fn run_width(&self, glyphs: &[GlyphId], scale: PxScale) -> f64 {
        let scaled = self.font.as_scaled(scale);
        let mut width = 0.0f32;
        let mut previous: Option<GlyphId> = None;
        for &glyph in glyphs {
            if let Some(previous) = previous {
                width += scaled.kern(previous, glyph);
            }
            width += scaled.h_advance(glyph);
            previous = Some(glyph);
        }
        f64::from(width)
    }

    /// How tall a line of text is at an em of `em` pixels.
    pub(crate) fn line_height(&self, em: f64) -> Option<f64> {
        let scaled = self.font.as_scaled(self.scale(em)?);
        Some(f64::from(scaled.ascent() - scaled.descent()))
    }

    /// How wide `text` is set at an em of `em` pixels.
    pub(crate) fn measure(&self, text: &str, em: f64) -> Option<f64> {
        let scale = self.scale(em)?;
        Some(self.run_width(&self.glyphs(text), scale))
    }

    /// `text`, or as much of it as fits in `width` followed by an ellipsis.
    pub(crate) fn fitted(&self, text: &str, em: f64, width: f64) -> Option<String> {
        let scale = self.scale(em)?;
        if self.run_width(&self.glyphs(text), scale) <= width + 0.5 {
            return Some(text.to_owned());
        }
        let mut kept: Vec<char> = text.chars().collect();
        while !kept.is_empty() {
            kept.pop();
            let candidate: String =
                kept.iter().collect::<String>().trim_end().to_owned() + "\u{2026}";
            if self.run_width(&self.glyphs(&candidate), scale) <= width + 0.5 {
                return Some(candidate);
            }
        }
        let ellipsis = ELLIPSIS.to_string();
        (self.run_width(&self.glyphs(&ellipsis), scale) <= width + 0.5).then_some(ellipsis)
    }

    /// Draws `text` in white, centred in `frame` on `surface`, cut short with
    /// an ellipsis when it is wider than the frame.
    pub(crate) fn draw(&self, surface: &mut Surface, text: &str, frame: &Rect, em: f64) {
        let (Some(scale), Some(text)) = (self.scale(em), self.fitted(text, em, frame.width)) else {
            return;
        };
        let scaled = self.font.as_scaled(scale);
        let glyphs = self.glyphs(&text);
        let width = self.run_width(&glyphs, scale);
        let line = f64::from(scaled.ascent() - scaled.descent());
        let mut x = (frame.x + (frame.width - width) / 2.0) as f32;
        let baseline = (frame.y + (frame.height - line) / 2.0) as f32 + scaled.ascent();
        let mut previous: Option<GlyphId> = None;
        for glyph in glyphs {
            if let Some(previous) = previous {
                x += scaled.kern(previous, glyph);
            }
            let positioned = glyph.with_scale_and_position(scale, point(x, baseline));
            x += scaled.h_advance(glyph);
            previous = Some(glyph);
            let Some(outline) = self.font.outline_glyph(positioned) else {
                continue;
            };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                surface.blend(
                    bounds.min.x as i32 + gx as i32,
                    bounds.min.y as i32 + gy as i32,
                    &WHITE,
                    f64::from(coverage),
                );
            });
        }
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
