use super::*;
use crate::system::process_identity::token_for_pid;
use crate::system::test_support::test_time::deadline;
use crate::tree::walk::walker_fake::ref_entry;
use agent_desktop_core::{ErrorCode, ProcessId, RefProcess};
use std::process::{Command, Stdio};

fn detail<'a>(error: &'a AdapterError, key: &str) -> Option<&'a serde_json::Value> {
    error.details.as_ref().and_then(|details| details.get(key))
}

fn own_pid() -> ProcessId {
    ProcessId::from(std::process::id())
}

fn own_token() -> String {
    token_for_pid(own_pid())
        .expect("the token read answers")
        .expect("a live process has a token")
}

fn desktop_window_id() -> String {
    let desktop =
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetDesktopWindow() } as isize;
    format!("w-{desktop}")
}

fn entry_for(pid: ProcessId, instance: Option<String>, window_id: Option<String>) -> RefEntry {
    let mut entry = ref_entry("button");
    entry.process = RefProcess {
        pid,
        process_instance: instance,
    };
    entry.source.source_window_id = window_id;
    entry
}

#[test]
fn a_ref_whose_window_belongs_to_another_process_never_resolves_into_it() {
    let entry = entry_for(own_pid(), Some(own_token()), Some(desktop_window_id()));

    let error = resolve_window_root(&entry, deadline(10_000))
        .err()
        .expect("a window another process owns must not be searched");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        detail(&error, "kind").and_then(serde_json::Value::as_str),
        Some("resolve_no_candidate")
    );
    assert!(error.permits_retry_by_default());
}

#[test]
fn a_recycled_pid_with_another_generation_is_classified_as_an_exited_owner() {
    let entry = entry_for(
        own_pid(),
        Some("not-the-live-generation".to_string()),
        Some(desktop_window_id()),
    );

    let error = verify_stored_failure_error(&entry, deadline(10_000));

    assert_eq!(
        detail(&error, "kind").and_then(serde_json::Value::as_str),
        Some("resolve_owning_process_exited")
    );
}

fn spawn_idle_child() -> std::process::Child {
    Command::new("ping")
        .args(["-n", "60", "127.0.0.1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("an idle child process starts")
}

#[test]
fn an_exited_owning_process_settles_terminal_and_not_retryable() {
    let mut child = spawn_idle_child();
    let pid = ProcessId::from(child.id());
    let token = token_for_pid(pid)
        .expect("the token read answers")
        .expect("a live child has a token");
    child.kill().expect("the child stops");
    child.wait().expect("the child is reaped");
    let entry = entry_for(pid, Some(token), Some(desktop_window_id()));

    let error = verify_stored_failure_error(&entry, deadline(10_000));

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        detail(&error, "kind").and_then(serde_json::Value::as_str),
        Some("resolve_owning_process_exited")
    );
    assert_eq!(
        detail(&error, "retryable").and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert!(
        !error.permits_retry_by_default(),
        "a dead owner must settle on the first attempt instead of burning the deadline"
    );
}

#[test]
fn a_live_owner_whose_window_moved_on_stays_retryable() {
    let entry = entry_for(own_pid(), Some(own_token()), Some(desktop_window_id()));

    let error = verify_stored_failure_error(&entry, deadline(10_000));

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        detail(&error, "kind").and_then(serde_json::Value::as_str),
        Some("resolve_no_candidate")
    );
    assert!(error.permits_retry_by_default());
}
