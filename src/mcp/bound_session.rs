//! The session this server created, passed to the calls that follow it.

use serde_json::Value;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::catalog::Tool;

/// The error detail the CLI attaches when a command names a session that has already ended.
const ENDED_KIND: &str = "session_ended";

/// The session an MCP server adopts from its own `desktop_session_start`.
///
/// A tool cannot pass a global flag, so a server started without `--session` ran every call outside any
/// session: `desktop_session_start` returned an id that no later call could act in, and a session-scoped
/// command such as `desktop_cursor_overlay_enable` failed with `INVALID_ARGS`. The server now adopts the
/// session it created and passes it on. Adopting only the server's own session also keeps a caller from
/// reaching a session id it never created, which is what a tool taking a `session` argument would allow.
///
/// A server started with a session of its own — `--session` or `AGENT_DESKTOP_SESSION` — keeps that one
/// and adopts nothing, because an operator that pinned a session chose it.
pub(crate) struct BoundSession {
    adopting: bool,
    id: Mutex<Option<String>>,
}

impl BoundSession {
    /// `pinned` is true when the server was started with a session of its own, by flag or environment.
    pub(crate) fn new(pinned: bool) -> Self {
        Self {
            adopting: !pinned,
            id: Mutex::new(None),
        }
    }

    /// The global flags a call runs with: the server's own, and the session it holds when it holds one.
    pub(crate) fn globals(&self, base: &[String]) -> Vec<String> {
        let mut globals = base.to_vec();
        if let Some(id) = self.bound() {
            globals.push(format!("--session={id}"));
        }
        globals
    }

    /// Adopts the id a `session start` returned, and releases the adopted id once that session ends: by
    /// this server's own `session end`, or by the `session_ended` refusal of a call that named it.
    ///
    /// The release matters because an ended session is not harmless: the CLI refuses every command that
    /// names one, so a session that ended behind this server's back would fail every call until it started
    /// another. An id that was never created is harmless and is left alone, since the CLI then simply runs
    /// without a session.
    pub(crate) fn observe(&self, tool: &Tool, envelope: &Value) {
        if !self.adopting {
            return;
        }
        if ended_session_refusal(envelope) {
            self.release_if(envelope["error"]["details"]["session_id"].as_str());
            return;
        }
        if envelope["ok"] != Value::Bool(true) {
            return;
        }
        let id = envelope["data"]["session_id"].as_str();
        if tool.path == ["session", "start"] {
            if let Some(id) = id {
                self.adopt(id);
            }
        } else if tool.path == ["session", "end"] {
            self.release_if(id);
        }
    }

    fn bound(&self) -> Option<String> {
        self.lock().clone()
    }

    fn adopt(&self, id: &str) {
        *self.lock() = Some(id.to_owned());
    }

    /// Releases the held session when it is `id`; an id that is unknown or different changes nothing.
    fn release_if(&self, id: Option<&str>) {
        let Some(id) = id else { return };
        let mut bound = self.lock();
        if bound.as_deref() == Some(id) {
            *bound = None;
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<String>> {
        self.id.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn ended_session_refusal(envelope: &Value) -> bool {
    envelope["ok"] == Value::Bool(false) && envelope["error"]["details"]["kind"] == ENDED_KIND
}

#[cfg(test)]
#[path = "bound_session_tests.rs"]
mod tests;
