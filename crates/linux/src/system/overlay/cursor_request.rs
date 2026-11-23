//! The cursor request for one overlay control, which the GNOME Shell
//! extension plays on Wayland and the renderer process plays on X11.
//! `disable` removes every cursor of the session, `hide` and `show` without
//! an agent id reach every cursor of the session, and every other request
//! reaches the exact session and agent pair.
use agent_desktop_core::{
    CursorImage, CursorMotion, CursorMotionProfile, CursorOverlayControl, CursorOverlayInstruction,
    CursorOverlayStyle, CursorPhase, Point, Rect,
};
use serde_json::{Value, json};

use super::cursor_frames::{FRAME_MS, effect_frames, travel_frames};
use super::cursor_start::CursorStart;

fn image(image: Option<&CursorImage>) -> Value {
    image.map_or(Value::Null, |image| {
        let hotspot = image.hotspot();
        json!({"path": image.path(), "hotspot": {"x": hotspot.x, "y": hotspot.y}})
    })
}

fn look(style: &CursorOverlayStyle) -> Value {
    json!({
        "fill": style.fill_rgb(),
        "rim": style.rim_rgb(),
        "accent": style.accent_rgb(),
        "size": style.size(),
        "arrow": image(style.image()),
        "pointer": image(style.pointer_image()),
    })
}

fn rect(rect: Option<&Rect>) -> Value {
    rect.map_or(
        Value::Null,
        |rect| json!({"x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height}),
    )
}

fn owned(op: &str, control: &CursorOverlayControl) -> Value {
    json!({"op": op, "session": control.session_id(), "agent": control.agent_id()})
}

/// The label the pill shows. The greeting an enable carries is left off a
/// cursor drawn from a custom image, as on macOS and Windows.
fn label(control: &CursorOverlayControl, style: &CursorOverlayStyle) -> Option<String> {
    let greeting_over_image = control.is_enable() && style.image().is_some();
    control
        .label()
        .filter(|_| !greeting_over_image)
        .map(str::to_owned)
}

/// Where a travel or a drag sends the cursor: a drag first goes to where it
/// begins.
pub(crate) fn travel_destination(instruction: &CursorOverlayInstruction) -> Point {
    match instruction.phase() {
        CursorPhase::Drag => instruction
            .drag_from()
            .unwrap_or_else(|| instruction.destination())
            .clone(),
        _ => instruction.destination().clone(),
    }
}

fn travel(
    request: &mut Value,
    instruction: &CursorOverlayInstruction,
    profile: &CursorMotionProfile,
    start: CursorStart,
) {
    let destination = travel_destination(instruction);
    let motion = CursorMotion::shaped(start.point(), destination, profile, start.moves)
        .with_impact(false)
        .with_ripple(false);
    request["frames"] = json!(travel_frames(&motion));
    request["frame_ms"] = json!(FRAME_MS);
    request["pointer"] = json!(instruction.is_pointer());
    request["clear_caret"] = json!(instruction.departs_from(Some(&start.point())));
    request["drag"] = json!(instruction.phase() == CursorPhase::Drag);
}

fn effect(
    request: &mut Value,
    instruction: &CursorOverlayInstruction,
    profile: &CursorMotionProfile,
    style: &CursorOverlayStyle,
) {
    let destination = instruction.destination().clone();
    let ripple = instruction.is_click() && style.ripple();
    let target = instruction.target().filter(|_| style.highlight());
    let motion = CursorMotion::shaped(destination.clone(), destination.clone(), profile, 0)
        .with_impact(instruction.is_click())
        .with_ripple(style.ripple());
    request["point"] = json!({"x": destination.x, "y": destination.y});
    request["target"] = rect(target);
    request["frames"] = json!(effect_frames(&motion, ripple, target.is_some()));
    request["frame_ms"] = json!(FRAME_MS);
}

/// The request for `control`. A travel or drag sets off from `start`, which
/// the extension reports; without one it starts on its destination.
pub(crate) fn cursor_request(control: &CursorOverlayControl, start: Option<CursorStart>) -> Value {
    if control.is_disable() {
        return json!({"op": "disable", "session": control.session_id()});
    }
    if control.is_hide() {
        return owned("hide", control);
    }
    if control.is_show() {
        return owned("show", control);
    }
    let default_style = CursorOverlayStyle::default();
    let style = control.style().unwrap_or(&default_style);
    let default_profile = CursorMotionProfile::default();
    let profile = control.motion().unwrap_or(&default_profile);
    let Some(instruction) = control.instruction() else {
        let mut request = owned("enable", control);
        request["look"] = look(style);
        request["label"] = json!(label(control, style));
        return request;
    };
    let op = match instruction.phase() {
        CursorPhase::Travel | CursorPhase::Drag => "travel",
        CursorPhase::Effect => "effect",
        CursorPhase::Caret | CursorPhase::CaretFollow => "caret",
    };
    let mut request = owned(op, control);
    request["look"] = look(style);
    request["label"] = json!(label(control, style));
    match instruction.phase() {
        CursorPhase::Travel | CursorPhase::Drag => {
            let start = start.unwrap_or_else(|| {
                let destination = travel_destination(instruction);
                CursorStart {
                    x: destination.x,
                    y: destination.y,
                    moves: 0,
                }
            });
            travel(&mut request, instruction, profile, start);
        }
        CursorPhase::Effect => effect(&mut request, instruction, profile, style),
        CursorPhase::Caret | CursorPhase::CaretFollow => {
            request["rect"] = rect(instruction.target());
            request["follow"] = json!(instruction.phase() == CursorPhase::CaretFollow);
        }
    }
    request
}

#[cfg(test)]
#[path = "cursor_request_tests.rs"]
mod tests;
