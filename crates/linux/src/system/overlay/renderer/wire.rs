//! One request and one reply per connection, each a single line of JSON.
//! A request names its `method`, `locate` or `update`, and carries the same
//! `body` the GNOME Shell extension takes. A reply is `ok` with a `value`,
//! or not `ok` with a `message`; `invalid` marks a request the renderer
//! refused rather than one it failed to play.
use std::io::{self, BufRead, BufReader, Read, Write};

use serde_json::{Value, json};

/// The longest request line the renderer reads, which holds the longest
/// path core's motion produces many times over.
pub(crate) const MAX_LINE_BYTES: u64 = 1024 * 1024;

/// Reads one line, without its newline. A line longer than the limit, or a
/// stream that ends before the newline, is an error.
pub(crate) fn read_line(stream: impl Read) -> io::Result<Value> {
    let mut line = Vec::new();
    BufReader::new(stream.take(MAX_LINE_BYTES + 1)).read_until(b'\n', &mut line)?;
    if line.pop() != Some(b'\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the cursor message is cut short or too long",
        ));
    }
    serde_json::from_slice(&line).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Writes `message` as one line.
pub(crate) fn write_line(mut stream: impl Write, message: &Value) -> io::Result<()> {
    let mut line = serde_json::to_vec(message).map_err(io::Error::other)?;
    line.push(b'\n');
    stream.write_all(&line)?;
    stream.flush()
}

pub(crate) fn request(method: &str, body: &Value) -> Value {
    json!({"method": method, "body": body})
}

pub(crate) fn success(value: Value) -> Value {
    json!({"ok": true, "value": value})
}

pub(crate) fn refusal(message: &str) -> Value {
    json!({"ok": false, "invalid": true, "message": message})
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
