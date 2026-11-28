//! Compact four-tool MCP surface grouping leaf commands into observe, act, window, and batch.

use serde_json::{Map, Value, json};
use std::hash::{BuildHasher, RandomState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::catalog::Tool;

static SCREENSHOT_COUNTER: AtomicU64 = AtomicU64::new(0);

const OBSERVE_TOOL: &str = "desktop_observe";
const ACT_TOOL: &str = "desktop_act";
const WINDOW_TOOL: &str = "desktop_window";
const BATCH_TOOL: &str = "desktop_batch";
const OBSERVE_BLOCKED_ARGS: &[&str] = &["output_path", "request", "guide", "headed"];

const OBSERVE_COMMANDS: &[&str] = &[
    "snapshot",
    "find",
    "get",
    "is",
    "screenshot",
    "list-apps",
    "list-windows",
    "list-displays",
    "list-surfaces",
    "list-notifications",
    "status",
    "permissions",
    "version",
    "skills list",
    "skills get",
    "skills path",
];

const ACT_COMMANDS: &[&str] = &[
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
    "clipboard-get",
    "clipboard-set",
    "clipboard-clear",
    "dismiss-notification",
    "dismiss-all-notifications",
    "notification-action",
];

const WINDOW_COMMANDS: &[&str] = &[
    "launch",
    "close-app",
    "focus-window",
    "resize-window",
    "move-window",
    "minimize",
    "maximize",
    "restore",
    "open-system-surface",
    "wait",
    "session start",
    "session end",
    "session list",
    "session gc",
];

pub(crate) const COMPACT_INSTRUCTIONS: &str = "\
agent-desktop reads native accessibility trees and operates desktop apps through four grouped MCP \
tools: desktop_observe (read-only observation, skills, status, and screenshots), desktop_act \
(element, keyboard, mouse, clipboard, and notification actions), desktop_window (app launch/close, \
window management, surfaces, waiting, and sessions), and desktop_batch (sequential multi-command \
batches). Pass `command` (for example \"snapshot\" or \"click\") plus command arguments either at \
 the top level or inside `args`. Call desktop_observe with {\"command\": \"skills get\", \"name\": \
\"desktop\"} first for the observe-act loop, and with {\"command\": \"skills get\", \"name\": \
\"platform\"} for OS-specific rules. Refs such as @s8f3k2p9:e1 come from snapshot and go stale \
when the UI changes: on STALE_REF or AMBIGUOUS_TARGET, snapshot again. Repeat an action only \
when its disposition.retry is \"safe\".";

pub(crate) fn definitions(leaf_tools: &[Tool]) -> Vec<Value> {
    let mut defs = vec![
        group_definition(
            OBSERVE_TOOL,
            "Read-only desktop observation: snapshot accessibility trees, find/get/is element state, screenshot windows, list apps/windows/surfaces/displays/notifications, check status/permissions, and read built-in skills",
            OBSERVE_COMMANDS,
            leaf_tools,
            true,
        ),
        group_definition(
            ACT_TOOL,
            "Desktop UI interaction: click, double-click, right-click, type, set-value, clear, focus, select, toggle/check/uncheck, expand/collapse, scroll, press key combos, mouse/drag, clipboard, and notification actions",
            ACT_COMMANDS,
            leaf_tools,
            false,
        ),
        group_definition(
            WINDOW_TOOL,
            "Application and window lifecycle: launch or close apps, focus/move/resize/minimize/maximize/restore windows, open shell surfaces, wait for conditions/events, and manage sessions",
            WINDOW_COMMANDS,
            leaf_tools,
            false,
        ),
    ];
    if let Some(batch) = leaf_tools.iter().find(|t| t.name == BATCH_TOOL) {
        defs.push(batch.definition());
    }
    defs
}

fn group_definition(
    tool_name: &str,
    description: &str,
    commands: &[&str],
    leaf_tools: &[Tool],
    read_only: bool,
) -> Value {
    let mut cmd_properties = Map::new();
    for cmd_label in commands {
        if let Some(leaf) = find_leaf_tool(leaf_tools, cmd_label) {
            let def = leaf.definition();
            if let Some(leaf_props) = def["inputSchema"]["properties"].as_object() {
                for (key, schema) in leaf_props {
                    if key == "args" || (read_only && OBSERVE_BLOCKED_ARGS.contains(&key.as_str()))
                    {
                        continue;
                    }
                    cmd_properties
                        .entry(key.clone())
                        .or_insert_with(|| schema.clone());
                }
            }
        }
    }
    let mut properties = Map::new();
    let enum_values = commands
        .iter()
        .map(|cmd| Value::from(*cmd))
        .collect::<Vec<_>>();
    properties.insert(
        "command".into(),
        json!({
            "type": "string",
            "enum": enum_values,
            "description": "The agent-desktop command to execute"
        }),
    );
    let args_schema = if tool_name == WINDOW_TOOL {
        json!({
            "description": "Optional nested command arguments object, or command-line argument array for `launch`",
            "oneOf": [
                {
                    "type": "object",
                    "properties": cmd_properties,
                    "additionalProperties": false
                },
                { "type": "array", "items": { "type": "string" } }
            ]
        })
    } else {
        json!({
            "type": "object",
            "description": "Optional nested command arguments object; arguments may also be passed as top-level properties",
            "properties": cmd_properties,
            "additionalProperties": false
        })
    };
    properties.insert("args".into(), args_schema);
    for (key, schema) in cmd_properties {
        properties.insert(key, schema);
    }
    json!({
        "name": tool_name,
        "description": description,
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": !read_only,
            "openWorldHint": false
        },
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": ["command"],
            "additionalProperties": false
        }
    })
}

pub(crate) type ResolvedCall<'a> = (&'a Tool, Map<String, Value>);

pub(crate) fn resolve_call<'a>(
    name: &str,
    arguments: &Map<String, Value>,
    leaf_tools: &'a [Tool],
) -> Result<Option<ResolvedCall<'a>>, String> {
    if name == BATCH_TOOL {
        let Some(batch) = leaf_tools.iter().find(|t| t.name == BATCH_TOOL) else {
            return Ok(None);
        };
        return Ok(Some((batch, arguments.clone())));
    }
    let allowed = match name {
        OBSERVE_TOOL => OBSERVE_COMMANDS,
        ACT_TOOL => ACT_COMMANDS,
        WINDOW_TOOL => WINDOW_COMMANDS,
        _ => return Ok(None),
    };
    let raw_cmd = arguments
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} requires a string 'command' argument"))?;
    let canonical = normalize_command_label(raw_cmd);
    if !allowed.iter().any(|cmd| *cmd == canonical) {
        return Err(format!(
            "Command '{raw_cmd}' is not valid for {name}. Allowed commands: {}",
            allowed.join(", ")
        ));
    }
    let leaf = find_leaf_tool(leaf_tools, &canonical)
        .ok_or_else(|| format!("Command '{canonical}' is not available"))?;
    let merged = extract_leaf_arguments(arguments, &canonical)?;
    if name == OBSERVE_TOOL {
        enforce_observe_read_only(&merged)?;
    }
    Ok(Some((leaf, merged)))
}

fn enforce_observe_read_only(arguments: &Map<String, Value>) -> Result<(), String> {
    if arguments.get("output_path").is_some_and(|v| !v.is_null()) {
        return Err(
            "desktop_observe is read-only and does not accept 'output_path'; omit it to return an inline image (or write into the server's --screenshot-dir)"
                .into(),
        );
    }
    if arguments.get("request") == Some(&Value::Bool(true)) {
        return Err(
            "desktop_observe is read-only and does not accept 'request: true' on permissions"
                .into(),
        );
    }
    if arguments.get("guide") == Some(&Value::Bool(true)) {
        return Err(
            "desktop_observe is read-only and does not accept 'guide: true' on permissions".into(),
        );
    }
    if arguments.get("headed").is_some_and(|v| !v.is_null()) {
        return Err("desktop_observe is read-only and does not accept 'headed'".into());
    }
    Ok(())
}

fn normalize_command_label(raw: &str) -> String {
    let trimmed = raw.trim().strip_prefix("desktop_").unwrap_or(raw.trim());
    match trimmed {
        "skills_list" | "skills-list" => "skills list".into(),
        "skills_get" | "skills-get" => "skills get".into(),
        "skills_path" | "skills-path" => "skills path".into(),
        "session_start" | "session-start" => "session start".into(),
        "session_end" | "session-end" => "session end".into(),
        "session_list" | "session-list" => "session list".into(),
        "session_gc" | "session-gc" => "session gc".into(),
        other if other.contains(' ') => other.to_string(),
        other => other.replace('_', "-"),
    }
}

fn find_leaf_tool<'a>(leaf_tools: &'a [Tool], command_label: &str) -> Option<&'a Tool> {
    let parts = command_label
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    leaf_tools.iter().find(|tool| tool.path == parts)
}

fn extract_leaf_arguments(
    arguments: &Map<String, Value>,
    canonical_command: &str,
) -> Result<Map<String, Value>, String> {
    let mut merged = Map::new();
    if let Some(nested) = arguments.get("args")
        && !nested.is_null()
    {
        match nested {
            Value::Object(obj) => {
                for (k, v) in obj {
                    merged.insert(k.clone(), v.clone());
                }
            }
            Value::Array(_) if canonical_command == "launch" => {
                merged.insert("args".into(), nested.clone());
            }
            _ => return Err("Argument 'args' must be an object".to_string()),
        }
    }
    for (k, v) in arguments {
        if k != "command" && k != "args" {
            if merged.contains_key(k) {
                return Err(format!(
                    "Argument '{k}' cannot be passed both at top level and inside 'args'"
                ));
            }
            merged.insert(k.clone(), v.clone());
        }
    }
    Ok(merged)
}

pub(crate) fn apply_default_screenshot_path(
    tool: &Tool,
    arguments: &mut Map<String, Value>,
    screenshot_dir: Option<&PathBuf>,
) {
    let Some(dir) = screenshot_dir else {
        return;
    };
    if tool.path != ["screenshot"] || arguments.get("output_path").is_some_and(|v| !v.is_null()) {
        return;
    }
    let now = SystemTime::now();
    let millis = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = SCREENSHOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nonce = RandomState::new().hash_one((std::process::id(), seq, now));
    let path = dir.join(format!("screenshot-{millis}-{nonce:016x}.png"));
    arguments.insert(
        "output_path".into(),
        Value::from(path.to_string_lossy().into_owned()),
    );
}

#[cfg(test)]
#[path = "compact_tests.rs"]
mod tests;
