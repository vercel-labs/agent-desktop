//! The renderer process: entered before argument parsing when the binary is
//! started as the overlay's child, so it never reaches the CLI. It takes the
//! renderer lock first, so a child that loses the race withdraws before it
//! draws anything, then connects to the display and listens.
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use agent_desktop_core::{AdapterError, ErrorCode};

use super::endpoint::Endpoint;
use super::painter::Painter;
use super::serve::Server;
use super::stage::Stage;
use super::text::LabelFont;

/// Marks a process started as the renderer. Its value is the protocol
/// generation, so a renderer from another build of the wire refuses to run.
pub(crate) const CHILD_MARKER: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_CHILD";
pub(crate) const GENERATION: &str = "x1";
/// The argument the renderer is started with. A process that inherited the
/// marker but was not started with this argument is an ordinary CLI run.
pub(crate) const CHILD_ARGUMENT: &str = "--cursor-overlay-child";
/// The X display the renderer draws on, as the CLI that started it chose it.
pub(crate) const DISPLAY_ENV: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_DISPLAY";

fn failure(message: impl std::fmt::Display) -> AdapterError {
    AdapterError::new(
        ErrorCode::Internal,
        format!("The cursor renderer could not start: {message}"),
    )
}

/// `Some` when this process was started as the renderer.
pub(crate) fn entry_from_env() -> Option<Result<(), AdapterError>> {
    super::spawn::mark_renderer_entry();
    let marker = std::env::var(CHILD_MARKER).ok()?;
    if std::env::args().nth(1).as_deref() != Some(CHILD_ARGUMENT) {
        return None;
    }
    if marker != GENERATION {
        return Some(Err(failure(
            "it was started for another protocol generation",
        )));
    }
    let Ok(display) = std::env::var(DISPLAY_ENV) else {
        return Some(Err(failure("no display was named")));
    };
    Some(run(&display))
}

fn run(display: &str) -> Result<(), AdapterError> {
    rustix::process::setsid().map_err(failure)?;
    let state_root = agent_desktop_core::session::agent_desktop_dir().map_err(failure)?;
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let endpoint = Endpoint::for_display(display, &state_root, runtime.as_deref());
    endpoint.ensure_directory().map_err(failure)?;
    let Some(_lock) = endpoint.try_lock().map_err(failure)? else {
        return Ok(());
    };
    let stage = Stage::open(display)?;
    stage.sweep_left_behind()?;
    match std::fs::remove_file(&endpoint.socket) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(failure(error)),
        _ => {}
    }
    let listener = endpoint
        .with_reachable_socket(|path| UnixListener::bind(path))
        .map_err(failure)?;
    std::fs::set_permissions(&endpoint.socket, std::fs::Permissions::from_mode(0o600))
        .map_err(failure)?;
    listener.set_nonblocking(true).map_err(failure)?;
    let served = Server::new(stage, listener, Painter::new(LabelFont::load())).run();
    let _ = std::fs::remove_file(&endpoint.socket);
    served
}
