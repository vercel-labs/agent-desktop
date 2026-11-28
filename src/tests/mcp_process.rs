use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
}

fn serve(requests: &[Value], args: &[&str]) -> Vec<Value> {
    let mut child = binary()
        .arg("mcp")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("binary starts");
    let mut stdin = child.stdin.take().expect("stdin");
    for request in requests {
        writeln!(stdin, "{request}").expect("request written");
    }
    drop(stdin);
    let output = child
        .wait_with_output()
        .expect("server exits after stdin closes");
    assert!(output.status.success());
    let mut responses = String::from_utf8(output.stdout)
        .expect("utf-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("one JSON-RPC message per line"))
        .collect::<Vec<_>>();
    responses.sort_by_key(|response| response["id"].as_i64());
    responses
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": arguments } })
}

/// Runs `requests` one at a time, reading the response before sending the next, so a call can rely on
/// what the call before it did: a session must have started before another call uses it.
fn serve_one_at_a_time(requests: &[Value], home: &Path) -> Vec<Value> {
    let mut child = binary()
        .arg("mcp")
        .env("AGENT_DESKTOP_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("binary starts");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut responses = std::io::BufReader::new(child.stdout.take().expect("stdout")).lines();
    let mut seen = Vec::new();
    for request in requests {
        writeln!(stdin, "{request}").expect("request written");
        stdin.flush().expect("request reaches the server");
        let line = responses
            .next()
            .expect("one response per request")
            .expect("utf-8 line");
        seen.push(serde_json::from_str::<Value>(&line).expect("one JSON-RPC message per line"));
    }
    drop(stdin);
    assert!(
        child
            .wait()
            .expect("server exits after stdin closes")
            .success()
    );
    seen
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .expect("create state root");
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) {
    std::fs::create_dir_all(dir).expect("create state root");
}

/// A state root of this test's own: `session start` refuses a parent that other users can reach.
fn private_home() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agent-desktop-mcp-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    create_private_dir(&dir);
    dir
}

#[test]
fn mcp_tool_calls_run_the_cli_command_and_return_its_envelope() {
    let responses = serve(
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": { "protocolVersion": "2025-11-25", "capabilities": {},
                                "clientInfo": { "name": "test", "version": "0" } } }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            call(2, "desktop_version", json!({})),
            call(
                3,
                "desktop_skills_get",
                json!({ "name": "desktop", "reference": null }),
            ),
        ],
        &[],
    );
    assert_eq!(responses.len(), 3, "{responses:?}");
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
    let version = &responses[1]["result"];
    assert_eq!(version["isError"], false, "{version}");
    assert_eq!(version["structuredContent"]["command"], "version");
    assert_eq!(
        version["structuredContent"]["data"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    let skill = &responses[2]["result"];
    assert_eq!(skill["isError"], false, "{skill}");
    assert!(
        skill["structuredContent"]["data"]["content"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
}

#[test]
fn mcp_server_global_flags_reach_every_call() {
    let responses = serve(
        &[call(1, "desktop_session_list", json!({}))],
        &["--debug", "--screenshot", "x.html"],
    );
    let result = &responses[0]["result"];
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(
        result["structuredContent"]["error"]["message"],
        "--debug supports snapshot and click only"
    );
}

#[test]
fn mcp_sandbox_exec_reaches_the_sandbox_when_the_server_has_globals() {
    let name = format!("missing-{}", std::process::id());
    let responses = serve(
        &[
            call(
                1,
                "desktop_sandbox_exec",
                json!({ "name": name, "args": ["snapshot", "--app", "Finder"] }),
            ),
            call(
                2,
                "desktop_sandbox_exec",
                json!({
                    "name": format!("{name}-guest"),
                    "args": ["--session=guest", "--headed", "snapshot", "--app", "Finder"],
                }),
            ),
        ],
        &[
            "--session=server",
            "--headed",
            "--trace=/tmp/mcp-sandbox-trace",
        ],
    );
    assert_eq!(responses.len(), 2, "{responses:?}");
    for response in &responses {
        let result = &response["result"];
        assert_eq!(result["isError"], true, "{result}");
        assert!(result.get("structuredContent").is_none(), "{result}");
        let envelope: Value = serde_json::from_str(
            result["content"][0]["text"]
                .as_str()
                .expect("exec passes the host's envelope through as text"),
        )
        .expect("the host's envelope is still JSON");
        assert_eq!(envelope["error"]["code"], "SANDBOX_NOT_FOUND", "{result}");
    }
}

#[test]
fn malformed_messages_get_errors_and_the_server_keeps_serving() {
    let responses = serve(
        &[
            json!([{ "jsonrpc": "2.0", "id": 9, "method": "tools/list" }]),
            json!(42),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "no/such/method" }),
            call(4, "desktop_skills_get", json!({ "name": "a\u{0}b" })),
            call(5, "desktop_version", json!({})),
        ],
        &[],
    );
    let codes = responses
        .iter()
        .map(|response| (response["id"].clone(), response["error"]["code"].clone()))
        .collect::<Vec<_>>();
    assert_eq!(codes[0], (Value::Null, json!(-32600)), "{responses:?}");
    assert_eq!(codes[1], (Value::Null, json!(-32600)), "{responses:?}");
    assert_eq!(codes[2], (json!(2), json!(-32601)), "{responses:?}");
    let nul = &responses[3]["result"];
    assert!(
        nul["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.starts_with("Cannot run agent-desktop")),
        "{nul}"
    );
    assert_eq!(responses[4]["result"]["isError"], false);
}

#[test]
fn a_session_the_server_started_is_the_one_the_calls_after_it_run_in() {
    let home = private_home();
    let responses = serve_one_at_a_time(
        &[
            call(1, "desktop_session_start", json!({})),
            call(2, "desktop_session_list", json!({})),
            call(3, "desktop_cursor_overlay_enable", json!({})),
        ],
        &home,
    );
    let _ = std::fs::remove_dir_all(&home);

    let started = &responses[0]["result"];
    assert_eq!(started["isError"], false, "{started}");
    let session_id = started["structuredContent"]["data"]["session_id"]
        .as_str()
        .expect("session start returns its id")
        .to_owned();

    let listed = &responses[1]["result"];
    assert_eq!(listed["isError"], false, "{listed}");
    let sessions = &listed["structuredContent"]["data"]["sessions"];
    assert_eq!(sessions[0]["session_id"], session_id, "{listed}");
    assert_eq!(
        sessions[0]["ended_at"],
        Value::Null,
        "the session the server started is still open: {listed}"
    );

    let enabled = &responses[2]["result"];
    assert_eq!(
        enabled["isError"], false,
        "a session this server started must be usable by the calls after it: {enabled}"
    );
    assert_eq!(
        enabled["structuredContent"]["data"]["session_id"],
        session_id
    );
}
