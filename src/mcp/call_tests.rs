use super::*;
use serde_json::json;
use std::path::Path;

fn tool_at(path: &[&str]) -> Tool {
    super::super::catalog::tools()
        .into_iter()
        .find(|tool| tool.path == path)
        .expect("tool exists")
}

fn child_args(tool: &Tool, arguments: Value, globals: &[&str]) -> Vec<String> {
    let arguments = arguments.as_object().expect("arguments object");
    let argv = tool.argv(arguments).expect("arguments build argv");
    let globals = globals.iter().map(ToString::to_string).collect::<Vec<_>>();
    child_command(Path::new("agent-desktop"), tool, argv, &globals)
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

/// A server started with a session of its own, so it adopts nothing and the mapping is what is tested.
fn pinned() -> BoundSession {
    BoundSession::new(true)
}

/// The globals a call runs with, borrowed as the child-command helper wants them.
fn globals_of(session: &BoundSession) -> Vec<String> {
    session.globals(&[])
}

#[test]
fn ordinary_tools_keep_server_globals_before_their_command() {
    let tool = tool_at(&["snapshot"]);
    let args = child_args(
        &tool,
        json!({ "app": "Finder" }),
        &["--session=abc", "--headed"],
    );
    assert_eq!(
        args,
        ["--session=abc", "--headed", "snapshot", "--app=Finder"]
    );
}

#[test]
fn a_tool_without_an_envelope_reports_its_stderr() {
    let tool = tool_at(&["version"]);
    let result = tool_result(
        &tool,
        b"",
        b"unexpected argument\n",
        "exit status: 2",
        false,
        &pinned(),
    );
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("unexpected argument")),
        "{result}"
    );
}

#[test]
fn sandbox_exec_gets_no_server_globals_and_keeps_guest_arguments_in_order() {
    let tool = tool_at(&["sandbox", "exec"]);
    let args = child_args(
        &tool,
        json!({ "name": "dev", "args": ["--headed", "type", "--", "@s8f3k2p9:e1", "--session=guest"] }),
        &["--session=server", "--trace=/tmp/host"],
    );
    assert_eq!(
        args,
        [
            "sandbox",
            "exec",
            "dev",
            "--",
            "--headed",
            "type",
            "--",
            "@s8f3k2p9:e1",
            "--session=guest",
        ]
    );
}

#[test]
fn every_tool_but_sandbox_exec_reports_a_json_envelope() {
    for tool in super::super::catalog::tools() {
        let expected = match tool.path == ["sandbox", "exec"] {
            true => Output::Passthrough,
            false => Output::Envelope,
        };
        assert_eq!(tool.output, expected, "{}", tool.name);
    }
}

#[test]
fn silent_guest_output_is_an_empty_success() {
    let tool = tool_at(&["sandbox", "exec"]);
    let result = tool_result(&tool, b"", b"", "exit status: 0", true, &pinned());
    assert_eq!(result["content"][0]["text"], "");
    assert_eq!(result["isError"], false);
    assert!(result.get("structuredContent").is_none(), "{result}");
}

#[test]
fn guest_json_is_passed_through_instead_of_being_read_as_the_host_envelope() {
    let tool = tool_at(&["sandbox", "exec"]);
    let result = tool_result(&tool, br#"{"a":1}"#, b"", "exit status: 0", true, &pinned());
    assert_eq!(result["content"][0]["text"], r#"{"a":1}"#);
    assert_eq!(result["isError"], false);
    assert!(result.get("structuredContent").is_none(), "{result}");
}

#[test]
fn a_failed_guest_keeps_its_output_and_reports_the_failure() {
    let tool = tool_at(&["sandbox", "exec"]);
    let result = tool_result(&tool, b"boom\n", b"", "exit status: 3", false, &pinned());
    assert_eq!(result["content"][0]["text"], "boom\n");
    assert_eq!(result["isError"], true);
}

#[test]
fn the_session_the_server_adopted_reaches_the_child_before_its_command() {
    let session = BoundSession::new(false);
    session.observe(
        &tool_at(&["session", "start"]),
        &json!({ "ok": true, "data": { "session_id": "run-1" } }),
    );
    let globals = globals_of(&session);
    let globals = globals.iter().map(String::as_str).collect::<Vec<_>>();
    assert_eq!(
        child_args(&tool_at(&["cursor-overlay", "enable"]), json!({}), &globals),
        ["--session=run-1", "cursor-overlay", "enable"]
    );
}

#[test]
fn sandbox_exec_drops_the_adopted_session_like_every_other_server_flag() {
    let session = BoundSession::new(false);
    session.observe(
        &tool_at(&["session", "start"]),
        &json!({ "ok": true, "data": { "session_id": "run-1" } }),
    );
    let globals = globals_of(&session);
    let globals = globals.iter().map(String::as_str).collect::<Vec<_>>();
    assert_eq!(
        child_args(
            &tool_at(&["sandbox", "exec"]),
            json!({ "name": "dev" }),
            &globals
        ),
        ["sandbox", "exec", "dev"]
    );
}
