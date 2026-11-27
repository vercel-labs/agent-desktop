use std::io::{ErrorKind, Read};
use std::process::{Command, Stdio};
use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};
use serde_json::Value;

use crate::system::display::display_mutter::LogicalMonitor;

const POLL: Duration = Duration::from_millis(10);

fn logical_size(width: i32, height: i32, scale: f64) -> (i32, i32) {
    if scale > 0.0 {
        (
            (f64::from(width) / scale).round() as i32,
            (f64::from(height) / scale).round() as i32,
        )
    } else {
        (width, height)
    }
}

fn parse_entry(entry: &Value) -> Option<LogicalMonitor> {
    if entry.get("connected")?.as_bool() != Some(true)
        || entry.get("enabled")?.as_bool() != Some(true)
    {
        return None;
    }
    let name = entry.get("name")?.as_str()?.to_owned();
    let pos = entry.get("pos")?;
    let x = i32::try_from(pos.get("x")?.as_i64()?).ok()?;
    let y = i32::try_from(pos.get("y")?.as_i64()?).ok()?;
    let size = entry.get("size")?;
    let raw_w = i32::try_from(size.get("width")?.as_i64()?).ok()?;
    let raw_h = i32::try_from(size.get("height")?.as_i64()?).ok()?;
    let scale = entry
        .get("scale")
        .and_then(Value::as_f64)
        .filter(|value| *value > 0.0)
        .unwrap_or(1.0);
    let (width, height) = logical_size(raw_w, raw_h, scale);
    if width <= 0 || height <= 0 {
        return None;
    }
    let primary = entry.get("priority").and_then(Value::as_u64) == Some(1);
    Some(LogicalMonitor {
        x,
        y,
        width,
        height,
        scale,
        primary,
        connectors: vec![name],
    })
}

pub(super) fn parse_outputs(raw: &str) -> Option<Vec<LogicalMonitor>> {
    let parsed: Value = serde_json::from_str(raw).ok()?;
    let outputs = parsed.get("outputs")?.as_array()?;
    let mut monitors: Vec<LogicalMonitor> = outputs.iter().filter_map(parse_entry).collect();
    if monitors.is_empty() {
        return None;
    }
    if !monitors.iter().any(|monitor| monitor.primary)
        && let Some(first) = monitors.first_mut()
    {
        first.primary = true;
    }
    Some(monitors)
}

fn run_kscreen_doctor(deadline: Deadline) -> Result<Option<String>, AdapterError> {
    deadline.ensure_remaining()?;
    let mut child = match Command::new("kscreen-doctor")
        .arg("-j")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Ok(None);
    };
    let reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        buf
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if deadline.is_expired() => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(deadline.timeout_error());
            }
            Ok(None) => std::thread::sleep(POLL.min(deadline.remaining())),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Ok(None);
            }
        }
    };
    let output = reader.join().unwrap_or_default();
    if !status.success() {
        return Ok(None);
    }
    Ok(Some(output))
}

/// Logical monitors reported by KDE's `kscreen-doctor -j`, or `None` when
/// `kscreen-doctor` is absent or reports no enabled outputs.
pub(crate) fn logical_monitors(
    deadline: Deadline,
) -> Result<Option<Vec<LogicalMonitor>>, AdapterError> {
    let Some(raw) = run_kscreen_doctor(deadline)? else {
        return Ok(None);
    };
    Ok(parse_outputs(&raw))
}

#[cfg(test)]
#[path = "display_kscreen_tests.rs"]
mod tests;
