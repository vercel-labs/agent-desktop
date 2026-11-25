use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, InteractionLease, InteractionPolicy, WindowState};

struct OneWindow {
    focused: bool,
    operated: AtomicU32,
    policy_seen: std::sync::Mutex<Option<InteractionPolicy>>,
}

fn window(focused: bool) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Fixture".into(),
        app: "Fixture".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("instance".into()),
        bounds: None,
        state: WindowState {
            is_focused: focused,
            ..Default::default()
        },
    }
}

impl ObservationOps for OneWindow {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![window(self.focused)])
    }
}

impl ActionOps for OneWindow {}

impl InputOps for OneWindow {}

impl SystemOps for OneWindow {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        win: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(win.clone())
    }

    fn window_op(
        &self,
        _win: &WindowInfo,
        _op: WindowOp,
        policy: InteractionPolicy,
        _lease: &InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        self.operated.fetch_add(1, Ordering::SeqCst);
        *self.policy_seen.lock().unwrap() = Some(policy);
        Ok(crate::adapter::test_report())
    }
}

fn adapter(focused: bool) -> OneWindow {
    OneWindow {
        focused,
        operated: AtomicU32::new(0),
        policy_seen: std::sync::Mutex::new(None),
    }
}

fn run(adapter: &OneWindow, op: WindowOp, policy: InteractionPolicy) -> Result<Value, AppError> {
    window_op_command(
        AppArgs {
            app: None,
            window_id: Some("w-1".into()),
        },
        adapter,
        op,
        policy,
    )
}

#[test]
fn headless_minimize_of_the_focused_window_is_refused_before_anything_is_sent() {
    let adapter = adapter(true);
    let error = run(&adapter, WindowOp::Minimize, InteractionPolicy::headless()).unwrap_err();
    assert_eq!(error.code(), "POLICY_DENIED");
    assert!(error.suggestion().unwrap().contains("--headed"));
    assert_eq!(adapter.operated.load(Ordering::SeqCst), 0);
}

#[test]
fn headed_minimize_of_the_focused_window_goes_through() {
    let adapter = adapter(true);
    run(&adapter, WindowOp::Minimize, InteractionPolicy::headed()).unwrap();
    assert_eq!(adapter.operated.load(Ordering::SeqCst), 1);
}

#[test]
fn headless_minimize_of_a_window_that_is_not_focused_goes_through() {
    let adapter = adapter(false);
    run(&adapter, WindowOp::Minimize, InteractionPolicy::headless()).unwrap();
    assert_eq!(adapter.operated.load(Ordering::SeqCst), 1);
}

#[test]
fn headless_maximize_of_the_focused_window_stays_headless() {
    let adapter = adapter(true);
    run(&adapter, WindowOp::Maximize, InteractionPolicy::headless()).unwrap();
    assert_eq!(
        *adapter.policy_seen.lock().unwrap(),
        Some(InteractionPolicy::headless())
    );
}
