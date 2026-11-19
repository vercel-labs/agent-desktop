//! The requests the renderer plays, checked the way the GNOME Shell extension
//! checks them: the CLI builds the same JSON for both, and anything malformed
//! is refused before a cursor changes.
use std::time::Duration;

use agent_desktop_core::{Point, Rect};
use serde_json::Value;

const MAX_SIZE: f64 = 8.0;
const MAX_FRAMES: usize = 600;
const MAX_FRAME_MS: f64 = 100.0;
const MAX_LABEL_CHARS: usize = 512;
const MAX_PATH_CHARS: usize = 1024;
const MAX_HOTSPOT: f64 = 512.0;

/// A custom cursor image and the point of it that sits on the tip.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Face {
    pub(crate) path: String,
    pub(crate) hotspot: Point,
}

/// How a cursor is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Look {
    pub(crate) fill: [f64; 3],
    pub(crate) rim: [f64; 3],
    pub(crate) accent: [f64; 3],
    pub(crate) size: f64,
    pub(crate) arrow: Option<Face>,
    pub(crate) pointer: Option<Face>,
}

/// The cursor a request is for: one agent of one session, or with no agent
/// the session's agent-less cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner {
    pub(crate) session: String,
    pub(crate) agent: Option<String>,
}

/// The parts every drawing request carries.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Styled {
    pub(crate) owner: Owner,
    pub(crate) look: Look,
    pub(crate) label: Option<String>,
}

/// Frames sampled from core's motion and the interval between them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Frames {
    pub(crate) frames: Vec<[f64; 2]>,
    pub(crate) frame_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Request {
    Conceal,
    Reveal,
    Disable(String),
    Hide(Owner),
    Show(Owner),
    Enable(Styled),
    Travel {
        styled: Styled,
        path: Frames,
        pointer: bool,
        clear_caret: bool,
        drag: bool,
    },
    Effect {
        styled: Styled,
        point: Point,
        target: Option<Rect>,
        path: Frames,
    },
    Caret {
        styled: Styled,
        rect: Option<Rect>,
        follow: bool,
    },
}

impl Frames {
    /// How long the frames take to play, as the extension times a travel.
    pub(crate) fn length(&self) -> Duration {
        let frames = self.frames.len().saturating_sub(1) as f64;
        Duration::from_millis((frames * self.frame_ms).round().max(0.0) as u64)
    }

    /// The frame shown `elapsed` after the first: picked by time, so a late
    /// frame changes smoothness and never the moment the last one shows.
    pub(crate) fn index_at(&self, elapsed: Duration) -> usize {
        let index = (elapsed.as_secs_f64() * 1000.0 / self.frame_ms).floor();
        (index.max(0.0) as usize).min(self.frames.len().saturating_sub(1))
    }

    /// The frame shown `elapsed` after the first.
    pub(crate) fn at(&self, elapsed: Duration) -> Option<[f64; 2]> {
        self.frames.get(self.index_at(elapsed)).copied()
    }
}

/// Where a travel to `destination` would set off for one cursor.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LocateRequest {
    pub(crate) owner: Owner,
    pub(crate) destination: Point,
}

fn invalid(message: impl std::fmt::Display) -> String {
    format!("Invalid cursor request: {message}")
}

/// Length as JavaScript counts it, so both renderers refuse the same strings.
fn utf16_length(text: &str) -> usize {
    text.encode_utf16().count()
}

fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}

fn color(value: &Value, name: &str) -> Result<[f64; 3], String> {
    let channel = |value: &Value| finite(value).filter(|c| (0.0..=1.0).contains(c));
    if let Some([r, g, b]) = value.as_array().map(Vec::as_slice)
        && let (Some(r), Some(g), Some(b)) = (channel(r), channel(g), channel(b))
    {
        return Ok([r, g, b]);
    }
    Err(invalid(format!(
        "{name} must be three channels between 0 and 1"
    )))
}

fn point(value: &Value, name: &str) -> Result<Point, String> {
    match (finite(&value["x"]), finite(&value["y"])) {
        (Some(x), Some(y)) => Ok(Point { x, y }),
        _ => Err(invalid(format!("{name} needs finite x and y"))),
    }
}

fn rect(value: &Value, name: &str) -> Result<Option<Rect>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let Point { x, y } = point(value, name)?;
    match (finite(&value["width"]), finite(&value["height"])) {
        (Some(width), Some(height)) if width >= 0.0 && height >= 0.0 => Ok(Some(Rect {
            x,
            y,
            width,
            height,
        })),
        _ => Err(invalid(format!(
            "{name} needs a finite, non-negative width and height"
        ))),
    }
}

fn face(value: &Value, name: &str) -> Result<Option<Face>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let path = value["path"]
        .as_str()
        .filter(|path| {
            path.starts_with('/') && utf16_length(path) <= MAX_PATH_CHARS && !path.contains('\0')
        })
        .ok_or_else(|| invalid(format!("{name} needs an absolute path")))?;
    let hotspot = point(&value["hotspot"], &format!("{name} hotspot"))?;
    if !(0.0..=MAX_HOTSPOT).contains(&hotspot.x) || !(0.0..=MAX_HOTSPOT).contains(&hotspot.y) {
        return Err(invalid(format!(
            "{name} hotspot must be between 0 and {MAX_HOTSPOT}"
        )));
    }
    Ok(Some(Face {
        path: path.to_owned(),
        hotspot,
    }))
}

fn look(value: &Value) -> Result<Look, String> {
    let size = finite(&value["size"])
        .filter(|size| *size > 0.0 && *size <= MAX_SIZE)
        .ok_or_else(|| invalid(format!("size must be above 0 and at most {MAX_SIZE}")))?;
    Ok(Look {
        fill: color(&value["fill"], "fill")?,
        rim: color(&value["rim"], "rim")?,
        accent: color(&value["accent"], "accent")?,
        size,
        arrow: face(&value["arrow"], "arrow")?,
        pointer: face(&value["pointer"], "pointer")?,
    })
}

fn label(value: &Value) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let text = value
        .as_str()
        .filter(|text| utf16_length(text) <= MAX_LABEL_CHARS)
        .ok_or_else(|| {
            invalid(format!(
                "label must be a string of at most {MAX_LABEL_CHARS} characters"
            ))
        })?;
    Ok((!text.trim().is_empty()).then(|| text.to_owned()))
}

fn owner(request: &Value) -> Result<Owner, String> {
    let session = request["session"]
        .as_str()
        .filter(|session| !session.is_empty())
        .ok_or_else(|| invalid("session must be a non-empty string"))?;
    let agent = match &request["agent"] {
        Value::Null => None,
        Value::String(agent) if !agent.is_empty() => Some(agent.clone()),
        _ => return Err(invalid("agent must be a non-empty string or null")),
    };
    Ok(Owner {
        session: session.to_owned(),
        agent,
    })
}

fn frames(request: &Value, least: usize, unit: bool) -> Result<Frames, String> {
    let in_range =
        |value: &Value| finite(value).filter(|number| !unit || (0.0..=1.0).contains(number));
    let listed = request["frames"]
        .as_array()
        .filter(|frames| (least..=MAX_FRAMES).contains(&frames.len()))
        .ok_or_else(|| invalid(format!("frames must hold {least} to {MAX_FRAMES} entries")))?;
    let frames = listed
        .iter()
        .map(|frame| match frame.as_array().map(Vec::as_slice) {
            Some([a, b]) => Some([in_range(a)?, in_range(b)?]),
            _ => None,
        })
        .collect::<Option<Vec<[f64; 2]>>>()
        .ok_or_else(|| invalid("every frame must be a pair of numbers in range"))?;
    let frame_ms = finite(&request["frame_ms"])
        .filter(|ms| *ms > 0.0 && *ms <= MAX_FRAME_MS)
        .ok_or_else(|| {
            invalid(format!(
                "frame_ms must be above 0 and at most {MAX_FRAME_MS}"
            ))
        })?;
    Ok(Frames { frames, frame_ms })
}

fn styled(request: &Value) -> Result<Styled, String> {
    Ok(Styled {
        owner: owner(request)?,
        look: look(&request["look"])?,
        label: label(&request["label"])?,
    })
}

/// One request from the CLI, checked.
pub(crate) fn parse_request(request: &Value) -> Result<Request, String> {
    match request["op"].as_str() {
        Some("conceal") => Ok(Request::Conceal),
        Some("reveal") => Ok(Request::Reveal),
        Some("disable") => Ok(Request::Disable(owner(request)?.session)),
        Some("hide") => Ok(Request::Hide(owner(request)?)),
        Some("show") => Ok(Request::Show(owner(request)?)),
        Some("enable") => Ok(Request::Enable(styled(request)?)),
        Some("travel") => Ok(Request::Travel {
            styled: styled(request)?,
            path: frames(request, 1, false)?,
            pointer: request["pointer"] == Value::Bool(true),
            clear_caret: request["clear_caret"] == Value::Bool(true),
            drag: request["drag"] == Value::Bool(true),
        }),
        Some("effect") => Ok(Request::Effect {
            styled: styled(request)?,
            point: point(&request["point"], "point")?,
            target: rect(&request["target"], "target")?,
            path: frames(request, 0, true)?,
        }),
        Some("caret") => Ok(Request::Caret {
            styled: styled(request)?,
            rect: rect(&request["rect"], "rect")?,
            follow: request["follow"] == Value::Bool(true),
        }),
        _ => Err(invalid(
            "op must be one of enable, travel, effect, caret, hide, show, disable, conceal, reveal",
        )),
    }
}

/// One locate request from the CLI, checked.
pub(crate) fn parse_locate(request: &Value) -> Result<LocateRequest, String> {
    Ok(LocateRequest {
        owner: owner(request)?,
        destination: point(&request["destination"], "destination")?,
    })
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
