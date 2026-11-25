use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

struct DoubleCheckAdapter {
    live_calls: AtomicU32,
    bounds_reads: AtomicU32,
    lease_held: Arc<AtomicBool>,
    prelease_calls: u32,
    timeout_ms: u64,
    expected_deadline: Option<crate::Deadline>,
    available_action: &'static str,
    surface_reads_under_lease: std::sync::Mutex<Vec<bool>>,
    census_delay: std::time::Duration,
    shared_lease: Option<Arc<AtomicBool>>,
    element_name: &'static str,
}

struct DoubleCheckLeaseGuard(Arc<AtomicBool>);

impl Drop for DoubleCheckLeaseGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl ObservationOps for DoubleCheckAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::adapter::LiveElement, AdapterError> {
        let call = self.live_calls.fetch_add(1, Ordering::SeqCst) + 1;
        assert!(
            self.shared_lease.is_some()
                || self.lease_held.load(Ordering::SeqCst) == (call > self.prelease_calls),
            "poll reads must be lease-free and final validation must be leased"
        );
        let states = if call <= self.prelease_calls {
            Vec::new()
        } else {
            vec!["disabled".to_string()]
        };
        Ok(crate::adapter::LiveElement {
            identity: crate::adapter::live_identity("Run"),
            state: crate::tree::element_state::ElementState {
                role: "button".into(),
                states,
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(bounds()),
            available_actions: vec![self.available_action.into()],
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        self.bounds_reads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(bounds()))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<crate::adapter::hit_test::HitTestResult, AdapterError> {
        Ok(crate::adapter::hit_test::HitTestResult::ReachesTarget)
    }
    crate::adapter::saved_ref_scope!(@inventory);
    crate::adapter::saved_ref_anchor!();

    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        if matches!(
            root,
            crate::locator::live_locator::ObservationRoot::Window(_)
        ) {
            std::thread::sleep(self.census_delay);
        }
        if let crate::locator::live_locator::ObservationRoot::Element { entry, .. } = &root {
            let mut node = crate::adapter::minimal_accessibility_node(&entry.identity.role, "");
            let leased = self
                .shared_lease
                .as_ref()
                .is_some_and(|shared| shared.load(Ordering::SeqCst));
            node.identity.name = Some(if leased { self.element_name } else { "Run" }.into());
            return crate::adapter::observed_tree(&root, node);
        }
        crate::adapter::saved_ref_tree(root, "button", Some("Run"))
    }

    fn list_surfaces(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::SurfaceInfo>, AdapterError> {
        self.surface_reads_under_lease
            .lock()
            .unwrap()
            .push(self.lease_held.load(Ordering::SeqCst));
        Ok(Vec::new())
    }
}

impl ActionOps for DoubleCheckAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::actions::action_result::ActionResult, AdapterError> {
        Ok(crate::actions::action_result::ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for DoubleCheckAdapter {}

impl SystemOps for DoubleCheckAdapter {
    crate::adapter::exact_window_focus!();

    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        if let Some(shared) = &self.shared_lease {
            while shared
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                if deadline.is_expired() {
                    return Err(
                        crate::session::interaction_lease::interaction_lease_timeout(
                            deadline,
                            serde_json::json!({}),
                        ),
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            return crate::InteractionLease::guarded(
                deadline,
                DoubleCheckLeaseGuard(Arc::clone(shared)),
            );
        }
        assert_eq!(self.live_calls.load(Ordering::SeqCst), self.prelease_calls);
        assert_eq!(deadline.timeout_ms(), self.timeout_ms);
        if let Some(expected) = self.expected_deadline {
            assert_eq!(deadline, expected);
            return Err(AdapterError::new(ErrorCode::Internal, "lease probe"));
        }
        self.lease_held.store(true, Ordering::SeqCst);
        Ok(crate::InteractionLease::guarded_with_contention(
            deadline,
            DoubleCheckLeaseGuard(Arc::clone(&self.lease_held)),
            3,
        ))
    }
}

/// Exactly one post-lease live read: this mock's bounds/hash never change,
/// so the permissive stability check that read establishes already passes -
/// `stable_preflight`'s settle-and-resample loop (`ref_action.rs`) exists
/// for a report whose only gap is `stable`, and short-circuits immediately
/// once the first live read already confirms it, rather than resampling a
/// target that was never unstable.
#[test]
fn stability_revalidates_once_under_lease_before_dispatch() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let adapter = adapter(2, 5_000, None);
    let result = run_with_entry(
        &adapter,
        ActionRequest::headed(Action::DoubleClick),
        5_000,
        &entry(),
    );

    assert_eq!(result.action, "click");
    assert_eq!(adapter.live_calls.load(Ordering::SeqCst), 3);
    let metrics = &result.details.as_ref().unwrap()["auto_wait"];
    assert_eq!(metrics["read_only_resolve_attempts"], 2);
    assert_eq!(metrics["read_only_preflight_attempts"], 2);
    assert_eq!(metrics["lease_contention_count"], 3);
    assert!(metrics["lease_hold_ms"].as_u64().is_some());
    assert!(!adapter.lease_held.load(Ordering::SeqCst));
}

/// The overlay read after a click that may open a window is a post-action
/// read: it must run once the interaction lease is released, so an
/// application that answers slowly while it opens a modal never holds the
/// lease for other agents.
#[test]
fn the_overlay_read_after_a_click_runs_after_the_lease_is_released() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let adapter = adapter(1, 5_000, None);
    run_with_entry(
        &adapter,
        ActionRequest::headless(Action::Click),
        5_000,
        &entry(),
    );

    assert_eq!(
        *adapter.surface_reads_under_lease.lock().unwrap(),
        vec![false]
    );
}

fn entry_with_mismatched_bounds_hash() -> RefEntry {
    let mut stale = entry();
    stale.geometry.bounds_hash = crate::Rect {
        x: 400.0,
        y: 400.0,
        width: 10.0,
        height: 10.0,
    }
    .bounds_hash();
    stale
}

/// A live element whose bounds and identity are otherwise fine, but whose
/// ref carries a `bounds_hash` from a different position - the exact shape
/// a caller's own preceding `scroll-to` on the same ref leaves behind
/// (measured live: -171 -> 181 on the Windows fixture, `A24-18`).
///
/// Before this fix, `stable_preflight`'s permissive check propagated this
/// mismatch as a terminal `ACTION_FAILED` via the `?` in `ref_action.rs`
/// before the settle-and-resample loop beneath it could ever run - proven
/// by reverting the discriminator and watching this test fail closed.
#[test]
fn a_stability_only_mismatch_settles_and_proceeds_to_dispatch() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let adapter = adapter(2, 5_000, None);
    let result = run_with_entry(
        &adapter,
        ActionRequest::headed(Action::DoubleClick),
        5_000,
        &entry_with_mismatched_bounds_hash(),
    );

    assert_eq!(result.action, "click");
    assert!(adapter.bounds_reads.load(Ordering::SeqCst) >= 3);
}

/// A headless pointerless action whose live bounds match the snapshot needs
/// one lease-free read, not a second observation to seed the comparison:
/// the leased read still validates it, exactly as for a semantic click.
#[test]
fn a_matching_snapshot_hash_settles_a_headless_pointerless_action_in_one_read() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let mut adapter = adapter(1, 5_000, None);
    adapter.available_action = capability::SCROLL;
    let result = run_with_entry(
        &adapter,
        ActionRequest::headless(Action::Scroll(crate::Direction::Down, 1)),
        5_000,
        &entry(),
    );

    assert_eq!(adapter.live_calls.load(Ordering::SeqCst), 2);
    let metrics = &result.details.as_ref().unwrap()["auto_wait"];
    assert_eq!(metrics["read_only_preflight_attempts"], 1);
}

fn run_with_entry(
    adapter: &DoubleCheckAdapter,
    request: ActionRequest,
    timeout_ms: u64,
    entry: &RefEntry,
) -> crate::actions::action_result::ActionResult {
    execute_with_auto_wait(
        RefActionWaitCtx {
            adapter,
            entry,
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        request.with_timeout_ms(Some(timeout_ms)),
    )
    .unwrap()
}

#[test]
fn lease_acquisition_receives_the_exact_original_deadline() {
    let deadline = crate::Deadline::after(80).unwrap();
    let adapter = adapter(0, 80, Some(deadline));
    let error = super::super::execute_with_deadline(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Click),
        deadline,
    )
    .err()
    .unwrap();

    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(error.message, "lease probe");
    assert_eq!(adapter.live_calls.load(Ordering::SeqCst), 0);
    assert!(!adapter.lease_held.load(Ordering::SeqCst));
}

fn adapter(
    prelease_calls: u32,
    timeout_ms: u64,
    expected_deadline: Option<crate::Deadline>,
) -> DoubleCheckAdapter {
    DoubleCheckAdapter {
        live_calls: AtomicU32::new(0),
        bounds_reads: AtomicU32::new(0),
        lease_held: Arc::new(AtomicBool::new(false)),
        prelease_calls,
        timeout_ms,
        expected_deadline,
        available_action: capability::CLICK,
        surface_reads_under_lease: std::sync::Mutex::new(Vec::new()),
        census_delay: std::time::Duration::ZERO,
        shared_lease: None,
        element_name: "Run",
    }
}

fn bounds() -> crate::Rect {
    crate::Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    }
}

#[path = "ref_action_wait_release_tests.rs"]
mod release_tests;
