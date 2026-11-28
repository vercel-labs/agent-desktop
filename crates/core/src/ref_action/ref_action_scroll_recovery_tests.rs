use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::ref_action::*;
use crate::{
    ErrorCode, actions::action::Action, actions::action_result::ActionResult, actions::capability,
    refs::RefEntry, snapshot::snapshot_surface::SnapshotSurface, tree::element_state::ElementState,
};
use std::sync::atomic::{AtomicU32, Ordering};

struct ScrollRecoveryAdapter {
    live_calls: AtomicU32,
    resolve_calls: AtomicU32,
    scroll_calls: AtomicU32,
    dispatch_calls: AtomicU32,
    stays_offscreen: bool,
    scroll_should_fail: bool,
    dispatch_fails: bool,
    scroll_unsupported: bool,
    refuses_route: bool,
    scroll_policy: std::sync::Mutex<Option<crate::InteractionPolicy>>,
}

impl ScrollRecoveryAdapter {
    fn new(stays_offscreen: bool, scroll_should_fail: bool) -> Self {
        Self {
            live_calls: AtomicU32::new(0),
            resolve_calls: AtomicU32::new(0),
            scroll_calls: AtomicU32::new(0),
            dispatch_calls: AtomicU32::new(0),
            stays_offscreen,
            scroll_should_fail,
            dispatch_fails: false,
            scroll_unsupported: false,
            refuses_route: false,
            scroll_policy: std::sync::Mutex::new(None),
        }
    }
}

impl ObservationOps for ScrollRecoveryAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_calls.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::null())
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::LiveElement, AdapterError> {
        let call = self.live_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let offscreen = self.stays_offscreen || call == 1;
        Ok(crate::LiveElement {
            identity: crate::adapter::live_identity("File"),
            state: ElementState {
                role: "menuitem".into(),
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(offscreen),
            },
            states_complete: false,
            bounds: Some(bounds()),
            available_actions: vec![capability::EXPAND.into()],
        })
    }
    crate::adapter::saved_ref_scope!("menuitem", Some("File"));
}

impl ActionOps for ScrollRecoveryAdapter {
    fn scroll_into_view(
        &self,
        _handle: &NativeHandle,
        policy: crate::InteractionPolicy,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        self.scroll_calls.fetch_add(1, Ordering::SeqCst);
        *self.scroll_policy.lock().unwrap() = Some(policy);
        if self.scroll_unsupported {
            return Err(AdapterError::not_supported("scroll_into_view"));
        }
        if self.scroll_should_fail {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "ScrollIntoView is not available on this element",
            )
            .with_suggestion(
                "scroll a containing viewport first, or target a scroll-item control",
            ));
        }
        Ok(())
    }

    fn refuse_before_scroll(
        &self,
        _handle: &NativeHandle,
        _request: &ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        if self.refuses_route {
            return Err(AdapterError::new(ErrorCode::ActionNotSupported, "no route"));
        }
        Ok(())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatch_calls.fetch_add(1, Ordering::SeqCst);
        if self.dispatch_fails {
            return Err(AdapterError::new(ErrorCode::ActionFailed, "write failed"));
        }
        Ok(ActionResult::delivered_unverified("expand"))
    }
}

impl InputOps for ScrollRecoveryAdapter {}
impl SystemOps for ScrollRecoveryAdapter {
    crate::adapter::guarded_interaction_lease!();
}

fn bounds() -> crate::Rect {
    crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    }
}

fn entry() -> RefEntry {
    let bounds = bounds();
    let mut entry = crate::adapter::minimal_ref_entry("menuitem", Some("File"));
    entry.geometry = crate::RefGeometry {
        bounds_space: None,
        bounds: Some(bounds),
        bounds_hash: bounds.bounds_hash(),
    };
    entry.capabilities.available_actions = vec![capability::EXPAND.into()];
    entry.source = crate::RefSource {
        source_app: Some("Notepad".into()),
        source_window_id: Some("w-1".into()),
        source_window_title: Some("Notepad".into()),
        source_window_bounds_hash: None,
        source_surface: SnapshotSurface::Window,
        renderer_accessibility_forced: false,
    };
    entry
}

#[test]
fn failed_scroll_recovery_surfaces_the_original_actionability_error() {
    let adapter = ScrollRecoveryAdapter::new(true, true);

    let error =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand)).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(
        error.message.contains("not actionable"),
        "expected the actionability diagnosis, got: {}",
        error.message
    );
    assert!(
        error.message.contains("visible"),
        "expected the failed visible check named in the message, got: {}",
        error.message
    );
    assert!(
        !error.message.contains("ScrollIntoView"),
        "the scroll recovery failure must not replace the actionability message, got: {}",
        error.message
    );
    let attempted = error
        .details
        .as_ref()
        .and_then(|details| details.get("scroll_into_view_attempted"))
        .expect("scroll attempt should be recorded as a detail on the original error");
    assert_eq!(
        attempted.get("message").and_then(serde_json::Value::as_str),
        Some("ScrollIntoView is not available on this element")
    );
    assert_eq!(adapter.scroll_calls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.dispatch_calls.load(Ordering::SeqCst), 0);
    assert_eq!(adapter.resolve_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn scroll_recovery_success_lets_the_action_proceed() {
    let adapter = ScrollRecoveryAdapter::new(false, false);

    let result = execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand))
        .expect("action should dispatch once the recovered target is actionable");

    assert_eq!(result.action, "expand");
    assert_eq!(adapter.scroll_calls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.dispatch_calls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.resolve_calls.load(Ordering::SeqCst), 2);
}

/// The recovery scroll runs under the policy the request carries, never a
/// hardcoded one. A cursor-only policy exercises the forwarding without the
/// window raise a headed preflight needs, which this stub cannot answer.
#[test]
fn scroll_recovery_runs_under_the_requests_interaction_policy() {
    let adapter = ScrollRecoveryAdapter::new(true, false);
    let policy = crate::InteractionPolicy {
        allow_focus_steal: false,
        allow_cursor_move: true,
    };
    let mut request = ActionRequest::headless(Action::Expand);
    request.policy = policy;

    execute_entry(&adapter, &entry(), request)
        .expect_err("a target that stays offscreen stays unactionable");

    let seen = adapter
        .scroll_policy
        .lock()
        .expect("the policy lock")
        .expect("the scroll ran");
    assert_eq!(seen, policy);
}

#[test]
fn scroll_recovery_that_leaves_the_target_unactionable_still_fails() {
    let adapter = ScrollRecoveryAdapter::new(true, false);

    let error =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand)).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(
        error.message.contains("not actionable"),
        "expected the second preflight's own diagnosis, got: {}",
        error.message
    );
    assert!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("scroll_into_view_attempted"))
            .is_none(),
        "a genuine second-preflight failure must not be dressed up as a failed recovery"
    );
    assert_eq!(adapter.scroll_calls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.dispatch_calls.load(Ordering::SeqCst), 0);
    assert_eq!(adapter.resolve_calls.load(Ordering::SeqCst), 2);
}

/// The recovery scroll is part of the caller's action, so it runs under the
/// caller's policy: a policy that permits the foreground lets the platform
/// use a route that needs it.
#[test]
fn scroll_recovery_runs_under_the_request_policy() {
    for request in [
        ActionRequest::headless(Action::Expand),
        ActionRequest::focus_fallback(Action::Expand),
    ] {
        let adapter = ScrollRecoveryAdapter::new(false, false);
        let policy = request.policy;
        execute_entry(&adapter, &entry(), request).expect("the recovered target dispatches");
        assert_eq!(*adapter.scroll_policy.lock().unwrap(), Some(policy));
    }
}

fn refusal_with_failed_visible(code: ErrorCode) -> AdapterError {
    AdapterError::new(code, "refused").with_details(serde_json::json!({
        "checks": [
            {"check": "visible", "status": "fail"},
            {"check": "supported_action", "status": "fail"}
        ]
    }))
}

#[test]
fn a_terminal_refusal_never_scrolls_the_view() {
    let request = ActionRequest::headless(Action::Expand);
    for code in [ErrorCode::PolicyDenied, ErrorCode::ActionNotSupported] {
        assert!(
            !crate::ref_action::ref_action_wait_evidence::should_scroll_after_preflight(
                &request,
                &refusal_with_failed_visible(code.clone()),
            ),
            "a {code:?} refusal must not scroll"
        );
    }
    assert!(
        crate::ref_action::ref_action_wait_evidence::should_scroll_after_preflight(
            &request,
            &refusal_with_failed_visible(ErrorCode::ActionFailed),
        )
    );
}

#[test]
fn a_dispatch_failure_after_a_scroll_reports_the_scroll() {
    let mut adapter = ScrollRecoveryAdapter::new(false, false);
    adapter.dispatch_fails = true;

    let error =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand)).unwrap_err();

    assert_eq!(adapter.scroll_calls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.dispatch_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("preparatory_scroll"))
            .and_then(serde_json::Value::as_str),
        Some("may_have_changed_view")
    );
    assert_eq!(error.disposition, crate::DeliverySemantics::uncertain());
}

#[test]
fn a_failed_scroll_attempt_reports_the_scroll() {
    let adapter = ScrollRecoveryAdapter::new(true, true);

    let error =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand)).unwrap_err();

    assert!(
        error
            .details
            .as_ref()
            .is_some_and(|details| details.get("preparatory_scroll").is_some())
    );
}

#[test]
fn a_final_refusal_is_decided_before_any_scroll() {
    let mut adapter = ScrollRecoveryAdapter::new(true, false);
    adapter.refuses_route = true;

    let error =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand)).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert_eq!(adapter.scroll_calls.load(Ordering::SeqCst), 0);
    assert_eq!(adapter.dispatch_calls.load(Ordering::SeqCst), 0);
    assert!(
        error
            .details
            .as_ref()
            .is_none_or(|details| details.get("preparatory_scroll").is_none())
    );
}

#[test]
fn a_not_delivered_failure_after_a_scroll_is_never_reported_as_not_delivered() {
    let adapter = ScrollRecoveryAdapter::new(true, false);

    let error =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand)).unwrap_err();

    assert_eq!(adapter.scroll_calls.load(Ordering::SeqCst), 1);
    assert_eq!(error.disposition, crate::DeliverySemantics::uncertain());
    assert_eq!(
        error.details.as_ref().unwrap()["preparatory_scroll"],
        "may_have_changed_view"
    );
}

#[test]
fn a_failed_scroll_attempt_does_not_claim_not_delivered() {
    let adapter = ScrollRecoveryAdapter::new(true, true);

    let error =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand)).unwrap_err();

    assert_eq!(error.disposition, crate::DeliverySemantics::uncertain());
}

#[test]
fn a_scroll_the_adapter_cannot_perform_leaves_the_refusal_not_delivered() {
    let mut adapter = ScrollRecoveryAdapter::new(true, false);
    adapter.scroll_unsupported = true;

    let error =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Expand)).unwrap_err();

    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert!(
        error
            .details
            .as_ref()
            .is_none_or(|details| details.get("preparatory_scroll").is_none())
    );
}
