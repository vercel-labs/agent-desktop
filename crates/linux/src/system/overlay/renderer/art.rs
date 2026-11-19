//! What one cursor window shows, described so that a window redraws only
//! when its description changes.
use agent_desktop_core::Rect;

use super::paint;
use super::picture::Picture;
use super::raster::Surface;
use super::text::LabelFont;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Art {
    Dart {
        scale: f64,
        fill: [f64; 3],
        rim: [f64; 3],
    },
    Face(Picture),
    Ripple {
        scale: f64,
        phase: f64,
        accent: [f64; 3],
    },
    /// The outline around a target `width` by `height`.
    Highlight {
        scale: f64,
        width: f64,
        height: f64,
        accent: [f64; 3],
    },
    /// The label pill `width` by `height`, its text set at `em` pixels in
    /// `text_frame`, which is relative to the pill's top-left corner.
    Pill {
        scale: f64,
        width: f64,
        height: f64,
        text: String,
        em: f64,
        text_frame: Rect,
    },
    Caret {
        width: f64,
        height: f64,
        accent: [f64; 3],
    },
}

impl Art {
    /// The pixels, or `None` when there is nothing to draw.
    pub(crate) fn render(&self, font: Option<&LabelFont>) -> Option<Surface> {
        match self {
            Self::Dart { scale, fill, rim } => paint::dart(*scale, *fill, *rim),
            Self::Face(picture) => Some(picture.surface().clone()),
            Self::Ripple {
                scale,
                phase,
                accent,
            } => paint::ripple(*scale, *phase, *accent),
            Self::Highlight {
                scale,
                width,
                height,
                accent,
            } => paint::highlight(*scale, *width, *height, *accent),
            Self::Pill {
                scale,
                width,
                height,
                text,
                em,
                text_frame,
            } => {
                let mut surface = paint::pill(*scale, *width, *height)?;
                let (side, _) = paint::shadow_margins(*scale);
                let frame = Rect {
                    x: text_frame.x + side,
                    y: text_frame.y + side,
                    ..*text_frame
                };
                font?.draw(&mut surface, text, &frame, *em);
                Some(surface)
            }
            Self::Caret {
                width,
                height,
                accent,
            } => paint::caret(*width, *height, *accent),
        }
    }
}
