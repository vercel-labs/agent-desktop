//! Starts the renderer: the agent-desktop binary itself, detached, with an
//! allowlisted environment and no inherited descriptors beyond null stdio.
use std::os::fd::BorrowedFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use agent_desktop_core::INTERACTION_LEASE_FD_ENV;
use rustix::io::{FdFlags, fcntl_getfd, fcntl_setfd};

use super::child::{CHILD_ARGUMENT, CHILD_MARKER, DISPLAY_ENV, GENERATION};

const INHERITED: [&str; 7] = [
    "DISPLAY",
    "XAUTHORITY",
    "XDG_RUNTIME_DIR",
    "AGENT_DESKTOP_HOME",
    "HOME",
    "PATH",
    "LANG",
];
const LOWEST_INHERITED_FD: i32 = 3;

static RENDERER_ENTRY: AtomicBool = AtomicBool::new(false);

/// Records that this process checked for the renderer argument. The CLI
/// does so before it parses its own arguments; an FFI host or a test binary
/// never does, so its image is never started as a renderer. The file name
/// proves nothing: npm installs the CLI as `agent-desktop-linux-x64` behind
/// an `agent-desktop` link.
pub(crate) fn mark_renderer_entry() {
    RENDERER_ENTRY.store(true, Ordering::Relaxed);
}

/// The image to start as the renderer: the running binary, once it has
/// shown that it enters the renderer.
pub(crate) fn image_for(entered: bool, running: std::io::Result<PathBuf>) -> Option<PathBuf> {
    entered.then(|| running.ok()).flatten()
}

/// The running binary when it can start the renderer.
pub(crate) fn renderer_image() -> Option<PathBuf> {
    image_for(
        RENDERER_ENTRY.load(Ordering::Relaxed),
        std::env::current_exe(),
    )
}

/// The command that starts the renderer for `display`.
pub(crate) fn build_command(
    image: &Path,
    display: &str,
    parent_env: &dyn Fn(&str) -> Option<String>,
) -> Command {
    let mut command = Command::new(image);
    command
        .arg(CHILD_ARGUMENT)
        .env_clear()
        .envs(
            INHERITED
                .into_iter()
                .filter_map(|name| parent_env(name).map(|value| (name, value))),
        )
        .env("DISPLAY", display)
        .env(CHILD_MARKER, GENERATION)
        .env(DISPLAY_ENV, display)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(lease) = parent_env(INTERACTION_LEASE_FD_ENV)
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|number| *number >= LOWEST_INHERITED_FD)
    {
        unsafe {
            command.pre_exec(move || withhold(lease));
        }
    }
    command
}

/// Keeps the interaction lease out of the renderer, which outlives the
/// command holding it. Runs in the forked child only.
fn withhold(descriptor: i32) -> std::io::Result<()> {
    let descriptor = unsafe { BorrowedFd::borrow_raw(descriptor) };
    match fcntl_getfd(descriptor) {
        Ok(flags) => {
            fcntl_setfd(descriptor, flags | FdFlags::CLOEXEC).map_err(std::io::Error::from)
        }
        Err(_) => Ok(()),
    }
}

/// Starts the renderer and collects its exit status in the background, so a
/// long-lived caller keeps no zombie.
pub(crate) fn start(image: &Path, display: &str) -> std::io::Result<()> {
    let mut child = build_command(image, display, &|name| std::env::var(name).ok()).spawn()?;
    std::thread::Builder::new()
        .name("cursor-renderer-reaper".to_owned())
        .spawn(move || {
            let _ = child.wait();
        })
        .map(|_| ())
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
