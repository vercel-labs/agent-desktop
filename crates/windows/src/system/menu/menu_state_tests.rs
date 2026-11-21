//! Windows-only: drives both measured menu sources (A23-1, A23-11) and their
//! union through forced seams and plain console processes. A23-10 grounds the
//! no-GUI-thread and nonexistent-pid cases.
#![cfg(target_os = "windows")]

use super::*;

use std::process::{Command, Stdio};

use crate::system::test_support::test_time::deadline;

#[test]
fn an_already_expired_deadline_returns_timeout_before_any_native_work() {
    thread_snapshot_calls::take();
    let expired = Deadline::after(0).expect("a zero-timeout deadline is constructible");

    let error =
        menu_is_open(ProcessId::from(std::process::id()), expired).expect_err("must refuse");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        thread_snapshot_calls::take(),
        0,
        "an already-expired deadline must perform no thread enumeration"
    );
}

#[test]
fn a_pid_with_no_gui_threads_reports_closed_rather_than_erroring() {
    let mut child = Command::new("cmd.exe")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("a bare console process starts");
    let pid = ProcessId::from(child.id());

    let open = menu_is_open(pid, deadline(10_000))
        .expect("a console-only process must not error the predicate");

    assert!(!open, "a process with no GUI thread has no menu open");

    let _ = child.kill();
    let _ = child.wait();
}

/// A snapshot handle opened inside the walk must be closed on the deadline
/// exit as well as the normal one. `wait --event` polls this path every
/// 200ms, so an exit that skips the close leaks one handle per expired
/// capture and exhausts the process over a long wait.
#[test]
fn an_expired_deadline_mid_walk_still_closes_the_snapshot_handle() {
    let _opens = thread_snapshot_calls::take();
    let _closes = thread_snapshot_closes::take();

    let expired = Deadline::after(1).expect("deadline builds");
    while !expired.is_expired() {
        std::thread::yield_now();
    }
    let outcome = classic_menu_mode_active(ProcessId::from(std::process::id()), expired);

    assert!(
        outcome.is_err(),
        "an expired deadline must refuse rather than complete the walk"
    );
    assert_eq!(
        thread_snapshot_calls::take(),
        thread_snapshot_closes::take(),
        "every ToolHelp snapshot opened by the walk must be closed on every exit path"
    );
}

/// The multi-target path takes one whole-desktop thread snapshot per call,
/// not one per target. `TH32CS_SNAPTHREAD` is unscoped, so a filter naming
/// several processes - two instances of one image name is ordinary - would
/// otherwise re-walk every thread on the desktop once per process on every
/// poll.
#[test]
fn menus_open_for_takes_one_snapshot_regardless_of_target_count() {
    let live: Vec<ProcessId> = crate::system::app_ops::process_snapshot()
        .expect("the process snapshot enumerates")
        .into_iter()
        .map(|row| row.pid)
        .filter(|pid| u32::from(*pid) != 0)
        .take(3)
        .collect();
    if live.len() < 2 {
        return;
    }

    let _drain = thread_snapshot_calls::take();
    let _outcome = menus_open_for(&live, deadline(10_000));

    assert_eq!(
        thread_snapshot_calls::take(),
        1,
        "{} targets must still cost exactly one thread snapshot",
        live.len()
    );
}

/// `CreateToolhelp32Snapshot` reports failure as `INVALID_HANDLE_VALUE`, which
/// is `-1`, never as a null handle. A guard written as `is_null()` cannot fire:
/// the failing call falls through, the enumeration loop never runs, and the
/// walk returns "no thread reported menu mode" - a masked native failure
/// reported as a confident negative. The classic source is the only one that
/// covers Win32 menu-bar dropdowns, so on that stack there is no second
/// opinion to catch it.
///
/// This drives the real guard with the value the API actually returns on
/// failure. Asserting facts about the constant alone would pass forever
/// regardless of what the guard compares against, which is the shape that let
/// the wrong guard survive three reviews.
#[test]
fn a_failed_snapshot_open_is_an_error_not_an_empty_enumeration() {
    let outcome = crate::system::runtime::thread_walk::force_open_failure::with(|| {
        menu_is_open(ProcessId::from(std::process::id()), deadline(10_000))
    });

    let error = outcome.expect_err(
        "a snapshot the OS refused to open must surface as an error, never as          a walk that completed and found no menu",
    );
    assert!(
        matches!(
            error.code,
            ErrorCode::AppUnresponsive | ErrorCode::Timeout | ErrorCode::ElementNotFound
        ),
        "the failure must stay inside the closed set the poll loop retries, got {:?}",
        error.code
    );
}

/// Split from this file so it stays inside the size cap.
#[path = "menu_state_agreement_tests.rs"]
mod agreement;
