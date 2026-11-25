use super::*;

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

/// The witness read that follows a delivered click is a post-action read: an
/// application that has just opened a modal does not answer it, and the read
/// waited for the whole operation deadline while the lease was held. The read
/// before the dispatch stays under the lease; the one after it runs after the
/// release and waits no longer than the witness budget.
#[test]
fn the_witness_read_after_a_click_runs_after_the_release_within_its_budget() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let adapter = adapter(1, 5_000, None);
    run_with_entry(
        &adapter,
        ActionRequest::headless(Action::Click),
        5_000,
        &entry(),
    );

    let reads = adapter.witness_reads.lock().unwrap().clone();
    assert_eq!(reads.len(), 2);
    assert!(
        reads[0].0,
        "the read before the dispatch is under the lease"
    );
    assert!(
        !reads[1].0,
        "the read after the dispatch is after the release"
    );
    let budget = 500;
    assert!(reads[0].1 <= budget && reads[1].1 <= budget, "{reads:?}");
}
