use agent_desktop_core::{AdapterError, Deadline};
use serde::Deserialize;

use super::{Unavailable, call};
use crate::tree::Extents;

/// One window as GNOME Shell manages it, in the compositor's logical
/// coordinates. `frame` is the visible window; `buffer` is the client's whole
/// surface, which also holds any client-side shadow.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct ShellWindow {
    pub(crate) pid: u32,
    pub(crate) title: String,
    pub(crate) wayland: bool,
    pub(crate) minimized: bool,
    pub(crate) frame: Extents,
    pub(crate) buffer: Extents,
}

/// Parses the extension's `ListWindows` reply.
pub(crate) fn parse_windows(json: &str) -> Result<Vec<ShellWindow>, AdapterError> {
    serde_json::from_str(json).map_err(|error| {
        AdapterError::internal("The agent-desktop GNOME Shell extension listed malformed windows")
            .with_platform_detail(error.to_string())
    })
}

/// Every window GNOME Shell manages, or why the extension could not say.
pub(crate) fn list_windows(
    deadline: Deadline,
) -> Result<Result<Vec<ShellWindow>, Unavailable>, AdapterError> {
    match call::<_, String>("ListWindows", &(), deadline)? {
        Ok(json) => parse_windows(&json).map(Ok),
        Err(unavailable) => Ok(Err(unavailable)),
    }
}

#[cfg(test)]
#[path = "extension_window_tests.rs"]
mod tests;
