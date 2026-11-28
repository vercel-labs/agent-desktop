use super::*;
use serde_json::json;

fn tool_at(path: &[&str]) -> Tool {
    super::super::catalog::tools()
        .into_iter()
        .find(|tool| tool.path == path)
        .expect("tool exists")
}

fn started(id: &str) -> Value {
    json!({ "ok": true, "command": "session", "data": { "session_id": id, "trace": "on" } })
}

fn ended(id: &str) -> Value {
    json!({ "ok": true, "command": "session", "data": { "session_id": id, "ended_at": 1 } })
}

fn adopt(session: &BoundSession, id: &str) {
    session.observe(&tool_at(&["session", "start"]), &started(id));
}

fn globals(session: &BoundSession) -> Vec<String> {
    session.globals(&[])
}

#[test]
fn a_session_start_this_server_ran_is_used_by_the_calls_after_it() {
    let session = BoundSession::new(false);
    assert!(globals(&session).is_empty());
    adopt(&session, "run-1");
    assert_eq!(globals(&session), ["--session=run-1"]);
}

#[test]
fn the_session_comes_after_the_flags_the_server_was_started_with() {
    let session = BoundSession::new(false);
    adopt(&session, "run-1");
    let base = ["--headed".to_string(), "--trace=/tmp/host".to_string()];
    assert_eq!(
        session.globals(&base),
        ["--headed", "--trace=/tmp/host", "--session=run-1"]
    );
}

#[test]
fn a_second_session_start_replaces_the_session_the_server_holds() {
    let session = BoundSession::new(false);
    adopt(&session, "run-1");
    adopt(&session, "run-2");
    assert_eq!(globals(&session), ["--session=run-2"]);
}

#[test]
fn a_server_started_with_a_session_of_its_own_adopts_nothing() {
    let session = BoundSession::new(true);
    adopt(&session, "run-1");
    assert!(globals(&session).is_empty());
}

#[test]
fn a_failed_session_start_adopts_nothing() {
    let session = BoundSession::new(false);
    let envelope = json!({
        "ok": false,
        "command": "session",
        "error": { "code": "INVALID_ARGS", "message": "private file parent is accessible by group" },
    });
    session.observe(&tool_at(&["session", "start"]), &envelope);
    assert!(globals(&session).is_empty());
}

#[test]
fn ending_the_adopted_session_releases_it() {
    let session = BoundSession::new(false);
    adopt(&session, "run-1");
    session.observe(&tool_at(&["session", "end"]), &ended("run-1"));
    assert!(globals(&session).is_empty());
}

#[test]
fn ending_another_session_keeps_the_adopted_one() {
    let session = BoundSession::new(false);
    adopt(&session, "run-1");
    session.observe(&tool_at(&["session", "end"]), &ended("run-2"));
    assert_eq!(globals(&session), ["--session=run-1"]);
}

#[test]
fn a_call_refused_because_the_adopted_session_ended_releases_it() {
    let session = BoundSession::new(false);
    adopt(&session, "run-1");
    let envelope = json!({
        "ok": false,
        "command": "snapshot",
        "error": {
            "code": "INVALID_ARGS",
            "details": { "kind": "session_ended", "session_id": "run-1" },
        },
    });
    session.observe(&tool_at(&["snapshot"]), &envelope);
    assert!(globals(&session).is_empty());
}

#[test]
fn a_call_refused_for_another_reason_keeps_the_adopted_session() {
    let session = BoundSession::new(false);
    adopt(&session, "run-1");
    let envelope = json!({
        "ok": false,
        "command": "snapshot",
        "error": {
            "code": "ELEMENT_NOT_FOUND",
            "details": { "kind": "surface_absent", "surface": "menu" },
        },
    });
    session.observe(&tool_at(&["snapshot"]), &envelope);
    assert_eq!(globals(&session), ["--session=run-1"]);
}

#[test]
fn ending_a_session_the_server_never_adopted_leaves_it_without_one() {
    let session = BoundSession::new(false);
    session.observe(&tool_at(&["session", "end"]), &ended("run-1"));
    assert!(globals(&session).is_empty());
}
