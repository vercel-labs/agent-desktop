//! Windows-only: proves the `wait_for_menu` poll loop's error handling and
//! target-death handling against forced seams and a windowless child process.
#![cfg(target_os = "windows")]

use super::*;

use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

const OPEN_AFTER: Duration = Duration::from_millis(300);
const RECV_TIMEOUT: Duration = Duration::from_secs(20);

fn deadline(timeout_ms: u64) -> Deadline {
    Deadline::after(timeout_ms).expect("bounded deadline")
}

fn identity_for(pid: ProcessId) -> ProcessIdentity {
    crate::system::test_support::live_identity::live_process_identity(pid)
}

/// The pid-recycling half of the identity discipline - a dead process whose
/// pid is reused by an unrelated live process before the next poll - cannot
/// be staged deterministically on demand, the same limitation
/// `menu_is_open_is_isolated_to_the_queried_process` in `menu_state_tests.rs`
/// documents for its own isolation property. What this test proves
/// deterministically is the half that a real termination does exercise: the
/// process-alive re-check the loop runs before every predicate read catches
/// the death and reports `StaleRef`, never a satisfied wait and never a
/// generic timeout.
///
/// The target is a windowless child ended with `TerminateProcess`, whose exit
/// is immediate. A GUI fixture closed through `WM_CLOSE` exits on its own
/// schedule, which on a loaded runner can outlast the exit confirmation slice
/// and turn the teardown into a legitimate `AppUnresponsive`; that
/// confirmation is covered with a staged clock in
/// `wait_exit_confirmation_tests.rs`.
#[test]
fn a_target_that_exits_mid_wait_returns_stale_ref_not_a_satisfied_wait_or_a_timeout() {
    let mut target = std::process::Command::new("ping")
        .args(["-n", "120", "127.0.0.1"])
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("the windowless target starts");
    let identity = identity_for(ProcessId::from(target.id()));

    let (sender, receiver) = channel();
    thread::spawn(move || {
        let result = wait_for_menu(identity, true, deadline(10_000));
        let _ = sender.send(result);
    });

    thread::sleep(OPEN_AFTER);
    target.kill().expect("the target is terminated");
    target.wait().expect("the target is reaped");

    let result = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("wait_for_menu completed");
    let error = result.expect_err("a terminated target must not report a satisfied wait");

    assert_eq!(error.code, ErrorCode::StaleRef, "{error:?}");
}

/// A target that dies mid-wait must carry the disposition and recovery an
/// agent acts on. `DeliverySemantics` defaults to unknown/unknown, so an
/// error built without an explicit disposition tells a caller nothing about
/// whether the wait delivered anything or whether retrying is safe - and this
/// failure is completely understood at the point it is constructed. The
/// project's own envelope contract says a consumer may read `recovery` only
/// when the retry disposition is `safe`, which a defaulted `unknown` never is.
#[test]
fn a_target_that_died_mid_wait_reports_a_disposition_and_a_recovery() {
    let error = stale_process_error(&ProcessIdentity::new(ProcessId::from(4321u32), "gone"));

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.disposition.delivery(),
        agent_desktop_core::DeliveryDisposition::NotDelivered,
        "a wait only observes, so a dead target delivered nothing"
    );
    assert!(
        error.suggestion.is_some(),
        "a dead target is a recoverable condition and must say how to recover"
    );
    let suggestion = error.suggestion.unwrap_or_default();
    assert!(
        !suggestion.contains("  "),
        "the suggestion is delivered verbatim in the JSON envelope, so a wrapped string          literal must not leave a run of indentation inside it, got {suggestion:?}"
    );
    assert!(
        !suggestion.contains("snapshot") && !suggestion.contains("ref"),
        "the recovery must be process-oriented; this surface has no refs or \
         snapshots to refresh, got {suggestion}"
    );
}

/// The predicate's own `Timeout` is deliberately absorbed and retried (the
/// loop's deadline check is the thing that should end that direction), but
/// every other error is a decided answer and must return on the first
/// occurrence. `forced_predicate_error` stands in for a real non-pumping
/// target the way `window_ops.rs`'s `force_window_not_found` stands in for a
/// real identity race elsewhere in this crate - both let a test pin the
/// loop's error-handling branch without staging the live condition. A
/// regression that widened the absorbed set to include this code, or that
/// kept retrying after the first non-`Timeout` error, would drive
/// `poll_calls` past one and the elapsed time toward the 10s deadline; this
/// test targets the current process, so no fixture or lock is needed.
#[test]
fn a_non_timeout_predicate_error_propagates_on_the_first_poll_without_spinning_to_the_deadline() {
    let pid = ProcessId::from(std::process::id());
    let token = process_identity::token_for_pid(pid)
        .expect("token read for this process")
        .expect("this process has a live process instance token");
    let identity = ProcessIdentity::new(pid, token);
    poll_calls::take();
    forced_predicate_error::arm(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "forced test predicate failure",
    ));

    let started = Instant::now();
    let result = wait_for_menu(identity, true, Deadline::after(10_000).expect("deadline"));

    let error = result.expect_err("a non-Timeout predicate error must propagate, not be absorbed");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        poll_calls::take(),
        1,
        "the loop must return on the first predicate read rather than retrying"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "a swallowed error would spin toward the 10s deadline instead of returning immediately, \
         elapsed {:?}",
        started.elapsed()
    );
}

/// Pins the terminal error arm's mapping in both directions without racing a
/// real process. `forced_predicate_error` supplies the `AppUnresponsive` that
/// `menu_state` returns for a target that vanished mid-read, and
/// `forced_process_death` makes the loop's own liveness re-check observe that
/// death after the pre-read check already passed. A regression that
/// propagated the predicate error without re-checking liveness would answer
/// the first case with the transient `AppUnresponsive` instead of the
/// contract's `StaleRef` and fail here, while the second case proves the same
/// error still propagates untouched when the target is alive.
#[test]
fn an_evaluation_error_surfaces_as_stale_ref_when_the_target_is_gone_and_unchanged_when_it_is_alive()
 {
    let pid = ProcessId::from(std::process::id());
    let token = process_identity::token_for_pid(pid)
        .expect("token read for this process")
        .expect("this process has a live process instance token");
    let identity = ProcessIdentity::new(pid, token);

    forced_predicate_error::arm(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "The target process could not be found",
    ));
    let alive = wait_for_menu(
        identity.clone(),
        true,
        Deadline::after(10_000).expect("deadline"),
    )
    .expect_err("a non-Timeout predicate error must propagate");
    assert_eq!(
        alive.code,
        ErrorCode::AppUnresponsive,
        "a live-but-unresponsive target must keep the predicate's own error"
    );
    assert_eq!(
        alive.message, "The target process could not be found",
        "the original error must propagate unchanged"
    );

    forced_process_death::arm_after(1);
    forced_predicate_error::arm(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "The target process could not be found",
    ));
    let gone = wait_for_menu(identity, true, Deadline::after(10_000).expect("deadline"))
        .expect_err("a target that exited mid-read must not report a satisfied wait");
    assert_eq!(
        gone.code,
        ErrorCode::StaleRef,
        "an evaluation error raised while the target is gone must surface as StaleRef"
    );
}

#[path = "wait_expiry_detail_tests.rs"]
mod expiry_detail_tests;

#[path = "wait_exit_confirmation_tests.rs"]
mod exit_confirmation_tests;
