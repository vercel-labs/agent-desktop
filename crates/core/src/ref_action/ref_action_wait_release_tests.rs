use super::*;

struct DisabledAfterLeaseAdapter {
    calls: AtomicU32,
    prelease_calls: u32,
    bounds_calls: AtomicU32,
}

impl ObservationOps for DisabledAfterLeaseAdapter {
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
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(crate::adapter::LiveElement {
            identity: crate::adapter::live_identity("Run"),
            state: crate::tree::element_state::ElementState {
                role: "button".into(),
                states: vec![],
                value: None,
                enabled: Some(call <= self.prelease_calls),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(bounds()),
            available_actions: vec![capability::CLICK.into()],
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        self.bounds_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(bounds()))
    }
    crate::adapter::saved_ref_scope!("button", Some("Run"));
}

impl ActionOps for DisabledAfterLeaseAdapter {}
impl InputOps for DisabledAfterLeaseAdapter {}
impl SystemOps for DisabledAfterLeaseAdapter {
    crate::adapter::exact_window_focus!();

    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        Ok(crate::InteractionLease::guarded_with_contention(
            deadline,
            DoubleCheckLeaseGuard(Arc::new(AtomicBool::new(false))),
            0,
        ))
    }
}

/// A stale bounds hash alongside a genuinely disabled element must not
/// enter the settle-and-resample loop: no amount of waiting enables a
/// disabled control, so the fix's discriminator - "stable" is the *only*
/// unmet requirement - must see "enabled" also fail and propagate
/// immediately, the same as before this fix existed. The element is
/// enabled for the pre-lease polling reads and only goes disabled once
/// `stable_preflight` reads it under lease, so this pins the post-lease
/// discriminator specifically rather than the separate pre-lease poll.
#[test]
fn a_disabled_element_fails_closed_without_sampling_even_with_a_stale_hash() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let adapter = DisabledAfterLeaseAdapter {
        calls: AtomicU32::new(0),
        prelease_calls: 2,
        bounds_calls: AtomicU32::new(0),
    };
    let error = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry_with_mismatched_bounds_hash(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headed(Action::DoubleClick).with_timeout_ms(Some(5_000)),
    )
    .err()
    .unwrap();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(adapter.bounds_calls.load(Ordering::SeqCst), 0);
}

/// A slow whole-window read is not part of what the lease protects. One agent's
/// action pauses inside its whole-window census before it can dispatch; a second
/// agent's action starts while the first is parked in that read and still gets
/// the lease and dispatches. Before the read moved ahead of the lease, the first
/// action held the lease through its read and the second timed out waiting.
#[test]
fn a_slow_whole_window_read_does_not_keep_a_parallel_action_from_the_lease() {
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let shared = Arc::new(AtomicBool::new(false));
    let mut slow = adapter(0, 5_000, None);
    slow.census_gate = Some((entered_tx, std::sync::Mutex::new(release_rx)));
    slow.shared_lease = Some(Arc::clone(&shared));
    let mut quick = adapter(0, 2_000, None);
    quick.shared_lease = Some(Arc::clone(&shared));

    let slow_action = std::thread::spawn(move || {
        execute_with_auto_wait(
            RefActionWaitCtx {
                adapter: &slow,
                entry: &entry(),
                ref_id: "@e1",
                context: &CommandContext::default(),
            },
            ActionRequest::headless(Action::Click).with_timeout_ms(Some(5_000)),
        )
        .is_ok()
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("slow action must enter whole-window census");
    let quick_result = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &quick,
            entry: &entry(),
            ref_id: "@e2",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Click).with_timeout_ms(Some(2_000)),
    );
    let _ = release_tx.send(());

    assert!(slow_action.join().unwrap());
    assert!(quick_result.is_ok(), "{quick_result:?}");
}

/// A live handle is not an identity. A virtualized list recycles its items for
/// other data after a scroll, so the handle the census proved can name a
/// different item by the time the lease is held. The check under the lease reads
/// the recorded identity fields again, so a handle whose name changed is refused
/// as stale and nothing is sent.
#[test]
fn a_handle_that_is_live_but_names_another_item_under_the_lease_is_stale_and_not_delivered() {
    let mut adapter = adapter(0, 5_000, None);
    adapter.shared_lease = Some(Arc::new(AtomicBool::new(false)));
    adapter.element_name = "Recycled row";

    let error = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Click).with_timeout_ms(Some(5_000)),
    )
    .expect_err("a recycled item must not be clicked");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
}
