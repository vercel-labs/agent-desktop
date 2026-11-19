//! Whether the sessions the renderer draws for are still live. A session can
//! end without a `disable` reaching the renderer: a crashed agent, a
//! `session gc`, an operator who stops. Its cursors then go by themselves.
//!
//! The manifest is read directly rather than through core's reader, which
//! turns an unreadable or half-written file into the same answer as a
//! deleted session. Here only absence counts, and only when two readings in
//! a row agree, the rule the Windows renderer keeps, so a manifest being
//! rewritten mid-session never ends a live overlay.
use std::collections::HashMap;
use std::io::{self, Read};

use serde_json::Value;

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// What one look at a session's manifest established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionReading {
    /// The session is live and its overlay is on.
    Live,
    /// The manifest is gone, its end is recorded, or its overlay is off.
    Finished,
    /// The manifest could not be read or parsed, which says nothing.
    Unknown,
}

pub(crate) fn classify(raw: io::Result<Vec<u8>>) -> SessionReading {
    let body = match raw {
        Ok(body) => body,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return SessionReading::Finished,
        Err(_) => return SessionReading::Unknown,
    };
    let Ok(value) = serde_json::from_slice::<Value>(&body) else {
        return SessionReading::Unknown;
    };
    if value.get("ended_at").is_some_and(|ended| !ended.is_null()) {
        return SessionReading::Finished;
    }
    match value.get("cursor_overlay") {
        None => SessionReading::Finished,
        Some(overlay) => match overlay.get("enabled").and_then(Value::as_bool) {
            Some(true) => SessionReading::Live,
            Some(false) => SessionReading::Finished,
            None => SessionReading::Unknown,
        },
    }
}

/// The manifest of `session`, read within a size limit.
pub(crate) fn read_manifest(session: &str) -> io::Result<Vec<u8>> {
    let directory = agent_desktop_core::session::session_dir(session)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let file =
        std::fs::File::open(directory.join(agent_desktop_core::session::SESSION_MANIFEST_FILE))?;
    let mut body = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut body)?;
    if body.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the session manifest exceeds the size limit",
        ));
    }
    Ok(body)
}

/// Counts each session's consecutive finished readings.
#[derive(Debug, Default)]
pub(crate) struct SessionWatch {
    finished: HashMap<String, u8>,
}

impl SessionWatch {
    /// Records `reading` for `session`, and answers whether the session has
    /// now read finished twice running. An unknown reading starts the count
    /// again.
    pub(crate) fn observe(&mut self, session: &str, reading: SessionReading) -> bool {
        if reading != SessionReading::Finished {
            self.finished.remove(session);
            return false;
        }
        let count = self.finished.entry(session.to_owned()).or_default();
        *count = count.saturating_add(1);
        *count >= 2
    }

    /// Forgets the sessions that no longer have cursors.
    pub(crate) fn keep_only(&mut self, sessions: &[String]) {
        self.finished
            .retain(|session, _| sessions.contains(session));
    }
}

#[cfg(test)]
#[path = "session_watch_tests.rs"]
mod tests;
