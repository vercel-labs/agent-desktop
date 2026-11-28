//! Optional per-call `--headed` and post-action `--wait-for` flag support when `--allow-headed` is enabled.

use serde_json::{Map, Value, json};

const HEADED_SUPPORTED_COMMANDS: &[&str] = &[
    "click",
    "double-click",
    "triple-click",
    "right-click",
    "type",
    "set-value",
    "clear",
    "focus",
    "select",
    "toggle",
    "check",
    "uncheck",
    "expand",
    "collapse",
    "scroll",
    "scroll-to",
    "press",
    "hover",
    "drag",
    "mouse-move",
    "mouse-click",
    "mouse-wheel",
    "launch",
    "close-app",
    "focus-window",
    "resize-window",
    "move-window",
    "minimize",
    "maximize",
    "restore",
    "open-system-surface",
    "dismiss-notification",
    "dismiss-all-notifications",
    "notification-action",
    "batch",
];

const WAIT_SUPPORTED_COMMANDS: &[&str] = &[
    "snapshot",
    "click",
    "double-click",
    "triple-click",
    "right-click",
    "type",
    "set-value",
    "clear",
    "focus",
    "select",
    "toggle",
    "check",
    "uncheck",
    "expand",
    "collapse",
    "scroll",
    "scroll-to",
    "press",
    "hover",
    "drag",
];

pub(crate) fn supports_headed(path: &[String]) -> bool {
    path.len() == 1
        && HEADED_SUPPORTED_COMMANDS
            .iter()
            .any(|cmd| *cmd == path[0].as_str())
}

pub(crate) fn supports_wait(path: &[String]) -> bool {
    path.len() == 1
        && WAIT_SUPPORTED_COMMANDS
            .iter()
            .any(|cmd| *cmd == path[0].as_str())
}

pub(crate) fn append_known_names(known: &mut Vec<String>, allow_headed: bool, supports_wait: bool) {
    if allow_headed {
        known.push("headed".into());
    }
    if supports_wait {
        known.extend([
            "wait_for".into(),
            "wait_for_gone".into(),
            "wait_timeout".into(),
        ]);
    }
}

pub(crate) fn insert_schema_properties(
    properties: &mut Map<String, Value>,
    allow_headed: bool,
    supports_wait: bool,
) {
    if allow_headed {
        properties.insert(
            "headed".into(),
            json!({
                "type": "boolean",
                "description": "Prefer physical input delivery and allow focus/cursor changes for this call"
            }),
        );
    }
    if supports_wait {
        properties.insert(
            "wait_for".into(),
            json!({
                "type": "string",
                "description": "Poll until an element matching role:text appears, then return the snapshot"
            }),
        );
        properties.insert(
            "wait_for_gone".into(),
            json!({
                "type": "string",
                "description": "Poll until no element matches role:text, then return the snapshot"
            }),
        );
        properties.insert(
            "wait_timeout".into(),
            json!({
                "type": "integer",
                "description": "Maximum wait time in milliseconds for wait_for / wait_for_gone (default: 30000)"
            }),
        );
    }
}

pub(crate) fn extract_argv(
    argv: &mut Vec<String>,
    arguments: &mut Map<String, Value>,
    allow_headed: bool,
    supports_wait: bool,
) -> Result<(), String> {
    if allow_headed && let Some(val) = arguments.remove("headed") {
        match val {
            Value::Null | Value::Bool(false) => {}
            Value::Bool(true) => argv.push("--headed".into()),
            _ => return Err("Argument 'headed' must be a boolean".into()),
        }
    }
    if !supports_wait {
        return Ok(());
    }
    if let Some(val) = arguments.remove("wait_for")
        && !val.is_null()
    {
        let text = val
            .as_str()
            .ok_or_else(|| "Argument 'wait_for' must be a string".to_string())?;
        argv.push(format!("--wait-for={text}"));
    }
    if let Some(val) = arguments.remove("wait_for_gone")
        && !val.is_null()
    {
        let text = val
            .as_str()
            .ok_or_else(|| "Argument 'wait_for_gone' must be a string".to_string())?;
        argv.push(format!("--wait-for-gone={text}"));
    }
    if let Some(val) = arguments.remove("wait_timeout")
        && !val.is_null()
    {
        let ms = val
            .as_u64()
            .ok_or_else(|| "Argument 'wait_timeout' must be a non-negative integer".to_string())?;
        argv.push(format!("--wait-timeout={ms}"));
    }
    Ok(())
}
