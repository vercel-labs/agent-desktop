//! What one cursor looks like at one instant, as a window per part: where
//! each part goes, what it shows and how opaque it is. The same placement
//! rules as the GNOME Shell extension, so a cursor looks alike on X11 and
//! Wayland.
use std::time::Instant;

use agent_desktop_core::{LABEL_FONT_POINTS, LabelPill, Point, place_label};

use super::art::Art;
use super::cursor::Cursor;
use super::desk::Desk;
use super::paint::{self, HIGHLIGHT_PADDING};
use super::piece::Piece;
use super::slot::Slot;
use super::text::LabelFont;

fn rounded(x: f64, y: f64) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

fn caret(cursor: &Cursor, opacity: f64) -> Option<Piece> {
    let rect = cursor.caret()?;
    Some(Piece {
        slot: Slot::Caret,
        origin: rounded(rect.x, rect.y),
        art: Art::Caret {
            width: rect.width.round().max(1.0),
            height: rect.height.round().max(1.0),
            accent: cursor.look().accent,
        },
        opacity,
    })
}

fn effect(cursor: &Cursor, tip: &Point, now: Instant, opacity: f64) -> Vec<Piece> {
    let Some(played) = cursor.playing_effect() else {
        return Vec::new();
    };
    let Some([phase, outline]) = played.at(now) else {
        return Vec::new();
    };
    let look = cursor.look();
    let mut pieces = Vec::new();
    if let Some(target) = played.target.as_ref().filter(|_| outline > 0.0) {
        let padding = HIGHLIGHT_PADDING * look.size;
        pieces.push(Piece {
            slot: Slot::Highlight,
            origin: rounded(target.x - padding, target.y - padding),
            art: Art::Highlight {
                scale: look.size,
                width: target.width,
                height: target.height,
                accent: look.accent,
            },
            opacity: opacity * outline,
        });
    }
    if phase > 0.0 && phase < 1.0 {
        let half = paint::ripple_side(look.size) / 2.0;
        pieces.push(Piece {
            slot: Slot::Ripple,
            origin: rounded(tip.x - half, tip.y - half),
            art: Art::Ripple {
                scale: look.size,
                phase,
                accent: look.accent,
            },
            opacity,
        });
    }
    pieces
}

/// Where the pill and its text go: right of and below the tip on the work
/// area that holds it, by core's `place_label`.
fn pill(cursor: &Cursor, tip: &Point, scenery: &Scenery, opacity: f64) -> Option<Piece> {
    let text = cursor.label().text.as_ref()?;
    let font = scenery.font?;
    let scale = cursor.look().size;
    let em = LABEL_FONT_POINTS * scale;
    let shaped = LabelPill::for_label(text, font.measure(text, em)?, scale)?;
    let placed = place_label(tip, shaped.size(), &scenery.desk.work_area_at(tip));
    let shaped = shaped.within(placed.width);
    let (side, _) = paint::shadow_margins(scale);
    let (x, y) = rounded(placed.x, placed.y);
    Some(Piece {
        slot: Slot::Pill,
        origin: (x - side as i32, y - side as i32),
        art: Art::Pill {
            scale,
            width: placed.width.ceil(),
            height: placed.height.ceil(),
            text: text.clone(),
            em,
            text_frame: shaped.text_frame(font.line_height(em)?),
        },
        opacity: opacity * cursor.label().opacity(scenery.now),
    })
}

fn glyph(cursor: &Cursor, tip: &Point, opacity: f64) -> Piece {
    let look = cursor.look();
    let scale = look.size;
    let (origin, art) = match cursor.faces().face(cursor.motion().pointing) {
        Some(picture) => {
            let hotspot = picture.hotspot();
            (
                rounded(tip.x - hotspot.x * scale, tip.y - hotspot.y * scale),
                Art::Face(picture.clone()),
            )
        }
        None => {
            let (x, y) = paint::dart_tip(scale);
            (
                rounded(tip.x - x, tip.y - y),
                Art::Dart {
                    scale,
                    fill: look.fill,
                    rim: look.rim,
                },
            )
        }
    };
    Piece {
        slot: Slot::Glyph,
        origin,
        art,
        opacity,
    }
}

/// What a scene is composed against.
pub(crate) struct Scenery<'a> {
    pub(crate) now: Instant,
    pub(crate) desk: &'a dyn Desk,
    pub(crate) font: Option<&'a LabelFont>,
}

/// The parts of `cursor` to show at the scenery's instant, bottom to top.
/// A hidden, concealed or fully rested cursor shows none.
pub(crate) fn compose(cursor: &Cursor, scenery: &Scenery) -> Vec<Piece> {
    let presence = cursor.presence();
    let opacity = presence.opacity(scenery.now);
    if !presence.is_shown() || opacity <= 0.0 {
        return Vec::new();
    }
    let mut pieces: Vec<Piece> = caret(cursor, opacity).into_iter().collect();
    if let Some(tip) = cursor.motion().tip.as_ref() {
        pieces.extend(effect(cursor, tip, scenery.now, opacity));
        pieces.extend(pill(cursor, tip, scenery, opacity));
        pieces.push(glyph(cursor, tip, opacity));
    }
    pieces
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
