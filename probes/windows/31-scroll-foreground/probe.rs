//! Scroll foreground-effect probe (A31).
//!
//! Measures whether `ScrollPattern.Scroll`, `ScrollPattern.SetScrollPercent`,
//! `ItemContainerPattern.FindItemByProperty` and `VirtualizedItemPattern.Realize`,
//! called through the product's CUIAutomation8 client on a window that is not in
//! the foreground, change the foreground window, keyboard focus, activation,
//! minimized state or z-order. `SetFocus` is the positive control and a name
//! read is the negative control. The `cli` mode puts the same bracket around
//! one run of a shipped `agent-desktop` binary.

use std::collections::HashMap;
use std::io::Write;

use serde_json::{Value, json};

#[cfg(target_os = "windows")]
#[path = "probe_state.rs"]
mod state;

#[cfg(target_os = "windows")]
#[path = "probe_hook.rs"]
mod hook;

#[cfg(target_os = "windows")]
#[path = "probe_calls.rs"]
mod calls;

#[cfg(target_os = "windows")]
#[path = "probe_run.rs"]
mod run;

struct Args(HashMap<String, String>);

impl Args {
    fn parse(raw: &[String]) -> Self {
        let mut map = HashMap::new();
        let mut index = 0;
        while index < raw.len() {
            let key = raw[index].trim_start_matches("--").to_string();
            let value = raw.get(index + 1).filter(|value| !value.starts_with("--")).cloned();
            index += if value.is_some() { 2 } else { 1 };
            map.insert(key, value.unwrap_or_else(|| "1".into()));
        }
        Self(map)
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn handle(&self, key: &str) -> Result<isize, String> {
        let text = self.get(key).ok_or_else(|| format!("missing --{key}"))?;
        let parsed = match text.strip_prefix("0x") {
            Some(hex) => isize::from_str_radix(hex, 16),
            None => text.parse(),
        };
        parsed.map_err(|error| format!("bad --{key} {text}: {error}"))
    }

    fn count(&self, key: &str, default: usize) -> usize {
        self.get(key).and_then(|value| value.parse().ok()).unwrap_or(default)
    }
}

fn digest_of(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(target_os = "windows")]
#[path = "probe_modes.rs"]
mod modes;

#[cfg(target_os = "windows")]
fn dispatch(mode: &str, args: &Args) -> Result<Value, String> {
    match mode {
        "tops" => modes::tops(),
        "find" => modes::find(args),
        "run" => modes::matrix(args),
        "act" => modes::act(args),
        "cli" => modes::cli(args),
        _ => Err(format!("unknown mode {mode:?}; modes: tops, find, run, act, cli")),
    }
}

#[cfg(not(target_os = "windows"))]
fn dispatch(_mode: &str, _args: &Args) -> Result<Value, String> {
    let _ = digest_of("");
    Ok(json!({ "skipped": "non-windows host" }))
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mode = raw.first().cloned().unwrap_or_default();
    let args = Args::parse(&raw[1.min(raw.len())..]);
    let doc = match dispatch(&mode, &args) {
        Ok(value) => value,
        Err(message) => json!({ "error": message }),
    };
    let failed = doc.get("error").is_some();
    let encoded = serde_json::to_string_pretty(&doc)
        .unwrap_or_else(|error| json!({ "serialize_failed": error.to_string() }).to_string());
    if let Some(path) = args.get("out") {
        if let Err(error) = std::fs::write(path, &encoded) {
            let _ = writeln!(std::io::stderr(), "out_write_failed={error}");
            std::process::exit(1);
        }
    }
    let _ = writeln!(std::io::stdout(), "{encoded}");
    if failed {
        std::process::exit(1);
    }
}
