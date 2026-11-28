use super::*;
use crate::system::close_outcome::CloseOutcome;
use crate::system::process_identity;
use agent_desktop_core::{DeliveryDisposition, ProcessId};

#[test]
fn protected_process_is_refused_before_any_native_close() {
    let app = AppInfo {
        aliases: Vec::new(),
        name: "explorer.exe".into(),
        pid: ProcessId::from(1u32),
        bundle_id: None,
        process_instance: Some("windows-proc-v1:1:1".into()),
        presentation: None,
    };
    let error = close_app_impl(&app, true, Deadline::after(1_000).expect("deadline"))
        .expect_err("protected");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(error.message.contains("protected"));
    assert!(error.suggestion.is_some());
}

#[test]
fn close_requires_a_creation_time_token() {
    let app = AppInfo {
        aliases: Vec::new(),
        name: "notepad.exe".into(),
        pid: ProcessId::from(1u32),
        bundle_id: None,
        process_instance: None,
        presentation: None,
    };
    let error = close_app_impl(&app, false, Deadline::after(1_000).expect("deadline"))
        .expect_err("token required");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[cfg(target_os = "windows")]
use crate::system::test_support::test_time::deadline;

#[cfg(target_os = "windows")]
fn app_for_pid(name: &str, pid: ProcessId) -> AppInfo {
    let token = process_identity::token_for_pid(pid)
        .expect("token read")
        .expect("live token");
    AppInfo {
        aliases: Vec::new(),
        name: name.into(),
        pid,
        bundle_id: None,
        process_instance: Some(token),
        presentation: None,
    }
}

#[cfg(target_os = "windows")]
fn process_still_alive(pid: ProcessId, instance: &str) -> bool {
    !super::process_observed_gone(pid, instance).expect("liveness")
}

#[cfg(target_os = "windows")]
use crate::system::lifecycle_test_support::spawn_windowless_child;

#[cfg(target_os = "windows")]
#[test]
fn already_dead_pid_is_benign_ok() {
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "exit", "/B", "0"])
        .spawn()
        .expect("spawn");
    let pid = ProcessId::from(child.id());
    let app = app_for_pid("cmd.exe", pid);
    let _ = child.wait();

    let outcome = close_app_impl(&app, false, deadline(10_000)).expect("already dead is Ok");

    assert_eq!(outcome, CloseOutcome::AlreadyGone);
}

#[cfg(target_os = "windows")]
#[test]
fn windowless_alive_graceful_is_action_failed_not_delivered_without_terminate() {
    let (_child, pid, token) = spawn_windowless_child();
    let app = AppInfo {
        aliases: Vec::new(),
        name: "cmd.exe".into(),
        pid,
        bundle_id: None,
        process_instance: Some(token.clone()),
        presentation: None,
    };
    let windows = super::top_level_windows_for_pid(pid).expect("enumerate");
    assert!(
        windows.is_empty(),
        "CREATE_NO_WINDOW child must present an empty top-level set"
    );

    let error = close_app_impl(&app, false, deadline(10_000)).expect_err("windowless alive");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(error.message.contains(&pid.to_string()));
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|text| text.contains("force") && text.contains(&pid.to_string()))
    );
    assert!(
        process_still_alive(pid, &token),
        "graceful windowless must never silent-TerminateProcess"
    );
}

/// `259` is `STILL_ACTIVE`, the same value `GetExitCodeProcess` reports for a
/// live process, so a process that legitimately exits with it must still be
/// observed gone. The wait signal is what proves the exit; vetoing on the code
/// would leave a cleanly exited process looking alive for as long as a handle
/// keeps it from being reaped - which the child handle held here does.
#[cfg(target_os = "windows")]
#[test]
fn an_exit_code_of_still_active_is_still_observed_as_gone() {
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "exit", "/B", "259"])
        .spawn()
        .expect("spawn");
    let pid = ProcessId::from(child.id());
    let token = process_identity::token_for_pid(pid)
        .expect("token read")
        .expect("live token");
    child.wait().expect("child exits");

    assert!(
        super::process_observed_gone(pid, &token).expect("liveness"),
        "a process that exited with 259 must not read as still running"
    );
}

#[test]
fn timeout_with_open_sheet_reports_dialog_pending_and_list_surfaces_suggestion() {
    let sheet = agent_desktop_core::SurfaceInfo {
        kind: agent_desktop_core::SnapshotSurface::Sheet
            .as_str()
            .to_string(),
        id: "w-123".to_string(),
        title: Some("Notepad".to_string()),
        item_count: None,
        unclassified: Vec::new(),
    };
    let timeout = Deadline::after(10).expect("deadline").timeout_error();
    let error = super::super::close_dispatch::timeout_with_sheets(
        timeout,
        "notepad.exe",
        ProcessId::from(42),
        false,
        &[sheet],
    );
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    let details = error.details.as_ref().expect("details");
    assert_eq!(details["cause"], "dialog_pending");
    assert_eq!(details["sheets"][0]["id"], "w-123");
    assert_eq!(details["sheets"][0]["title"], "Notepad");
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|s| s.contains("list-surfaces"))
    );
}
