//! The CLI side of the renderer: it finds or starts the renderer for the
//! display and sends it the same requests the GNOME Shell extension takes.
use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use agent_desktop_core::{AdapterError, CursorOverlayControl, Deadline, ErrorCode, Point};
use serde_json::{Value, json};

use super::endpoint::Endpoint;
use super::spawn;
use super::wire;
use crate::system::overlay::cursor_request::{cursor_request, travel_destination};
use crate::system::overlay::cursor_start::CursorStart;

const FIRST_PAUSE: Duration = Duration::from_millis(5);
const LONGEST_PAUSE: Duration = Duration::from_millis(50);

/// Why no renderer took a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unreachable {
    /// No renderer runs, and the request is not one that starts it.
    NotRunning,
    /// This process cannot start the renderer: it is not the agent-desktop
    /// binary, such as an FFI host.
    NotStartable,
}

fn endpoint(display: &str) -> Result<Endpoint, AdapterError> {
    let state_root = agent_desktop_core::session::agent_desktop_dir().map_err(|error| {
        AdapterError::internal("The state root could not be resolved")
            .with_platform_detail(error.to_string())
    })?;
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    Ok(Endpoint::for_display(
        display,
        &state_root,
        runtime.as_deref(),
    ))
}

fn unreachable_server(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

fn connection_error(error: impl std::fmt::Display) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The X11 cursor renderer did not answer",
    )
    .with_platform_detail(error.to_string())
}

/// Connects to the renderer, starting it when `start` allows and none runs.
fn connect(
    display: &str,
    start: bool,
    deadline: Deadline,
) -> Result<Result<UnixStream, Unreachable>, AdapterError> {
    let endpoint = endpoint(display)?;
    let reach = || endpoint.with_reachable_socket(|path| UnixStream::connect(path));
    match reach() {
        Ok(stream) => return Ok(Ok(stream)),
        Err(error) if !unreachable_server(&error) => return Err(connection_error(error)),
        Err(_) if !start => return Ok(Err(Unreachable::NotRunning)),
        Err(_) => {}
    }
    let Some(image) = spawn::renderer_image() else {
        return Ok(Err(Unreachable::NotStartable));
    };
    spawn::start(&image, display).map_err(connection_error)?;
    let mut pause = FIRST_PAUSE;
    loop {
        match reach() {
            Ok(stream) => return Ok(Ok(stream)),
            Err(error) if !unreachable_server(&error) => return Err(connection_error(error)),
            Err(error) if deadline.is_expired() => {
                return Err(
                    AdapterError::timeout("The X11 cursor renderer did not start in time")
                        .with_platform_detail(error.to_string()),
                );
            }
            Err(_) => {}
        }
        std::thread::sleep(pause.min(deadline.remaining()));
        pause = (pause * 2).min(LONGEST_PAUSE);
    }
}

/// Sends one request on `stream` and reads the renderer's answer.
fn exchange(
    stream: &UnixStream,
    method: &str,
    body: &Value,
    deadline: Deadline,
) -> Result<Value, AdapterError> {
    let remaining = deadline.remaining().max(Duration::from_millis(1));
    stream
        .set_read_timeout(Some(remaining))
        .map_err(connection_error)?;
    stream
        .set_write_timeout(Some(remaining))
        .map_err(connection_error)?;
    wire::write_line(stream, &wire::request(method, body)).map_err(connection_error)?;
    let reply = wire::read_line(stream).map_err(|error| match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
            AdapterError::timeout("The X11 cursor renderer did not answer in time")
        }
        _ => connection_error(error),
    })?;
    if reply["ok"] == Value::Bool(true) {
        return Ok(reply["value"].clone());
    }
    let message = reply["message"]
        .as_str()
        .unwrap_or("The X11 cursor renderer refused the request");
    let code = if reply["invalid"] == Value::Bool(true) {
        ErrorCode::InvalidArgs
    } else {
        ErrorCode::ActionFailed
    };
    Err(AdapterError::new(code, message.to_owned()))
}

/// Sends one request to the renderer for `display`, starting it first when
/// `start` allows and none runs.
pub(crate) fn call(
    display: &str,
    method: &str,
    body: &Value,
    start: bool,
    deadline: Deadline,
) -> Result<Result<Value, Unreachable>, AdapterError> {
    match connect(display, start, deadline)? {
        Ok(stream) => exchange(&stream, method, body, deadline).map(Ok),
        Err(unreachable) => Ok(Err(unreachable)),
    }
}

/// Whether `request` may start a renderer: only one that draws a cursor. A
/// request that hides, shows, removes or conceals cursors has nothing to do
/// when no renderer runs.
fn starts_renderer(request: &Value) -> bool {
    match request["op"].as_str() {
        Some("enable" | "travel" | "effect") => true,
        Some("caret") => request["follow"] != Value::Bool(true),
        _ => false,
    }
}

fn locate(
    display: &str,
    control: &CursorOverlayControl,
    destination: &Point,
    deadline: Deadline,
) -> Result<Result<CursorStart, Unreachable>, AdapterError> {
    let request = json!({
        "session": control.session_id(),
        "agent": control.agent_id(),
        "destination": {"x": destination.x, "y": destination.y},
    });
    let reply = match call(display, "locate", &request, true, deadline)? {
        Ok(reply) => reply,
        Err(unreachable) => return Ok(Err(unreachable)),
    };
    serde_json::from_value(reply).map(Ok).map_err(|error| {
        AdapterError::internal("The X11 cursor renderer answered malformed data")
            .with_platform_detail(error.to_string())
    })
}

/// Draws one overlay control through the renderer. A travel first asks
/// where the cursor stands, so the path is core's motion from there. When no
/// renderer took the control, the caller plays it on the in-process overlay,
/// which also reaches windows an earlier version left on the server.
pub(crate) fn update_cursor(
    display: &str,
    control: &CursorOverlayControl,
    deadline: Deadline,
) -> Result<Result<(), Unreachable>, AdapterError> {
    let start = match control.instruction().filter(|_| control.is_travel()) {
        Some(instruction) => {
            match locate(display, control, &travel_destination(instruction), deadline)? {
                Ok(start) => Some(start),
                Err(unreachable) => return Ok(Err(unreachable)),
            }
        }
        None => None,
    };
    let request = cursor_request(control, start);
    Ok(call(
        display,
        "update",
        &request,
        starts_renderer(&request),
        deadline,
    )?
    .map(|_| ()))
}

/// Runs `capture` with every renderer cursor hidden and shows them again
/// afterwards. With no renderer there is nothing to hide. The renderer
/// shows them again by itself if this process never asks.
pub(crate) fn with_concealed<T>(
    display: &str,
    deadline: Deadline,
    capture: impl FnOnce() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    let concealed = (call(
        display,
        "update",
        &json!({"op": "conceal"}),
        false,
        deadline,
    )?)
    .is_ok();
    let result = capture();
    if concealed {
        let cleanup = Deadline::detached_after(1_500)?;
        let _gone_renderer_has_nothing_to_reveal =
            call(display, "update", &json!({"op": "reveal"}), false, cleanup)?;
    }
    result
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
