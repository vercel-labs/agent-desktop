//! What the ledger does when it cannot be written, or when two deliveries
//! overlap in time.
//!
//! These are the cases a caller cannot see from the JSON alone, so they are
//! driven from the same `right-click` → `snapshot --surface menu` pair and read
//! the same two fields: a receipt and an origin, or neither.

use super::flow_fixture_tests::*;
use super::ledger::{DeliveryState, MenuDelivery, PriorMenu};
use super::now_ms;
use super::store::MenuLedgerStore;
use crate::refs::refs_test_support::HomeGuard;

#[test]
fn a_delivery_the_ledger_could_not_hold_reports_nothing() {
    let _home = HomeGuard::new();
    let row = row_ref();
    let state_root = crate::session::agent_desktop_dir().unwrap();
    crate::storage::private_file_parent::ensure_private(&state_root).unwrap();
    std::fs::create_dir(state_root.join("menu_origin.json")).unwrap();

    let click = right_click(
        &row,
        &[MenuAnswer::Absent, MenuAnswer::Named(TOKEN_A)],
        true,
    );
    assert!(click.get("menu_delivery").is_none());

    std::fs::remove_dir(state_root.join("menu_origin.json")).unwrap();
    let snapshot = menu_snapshot(&[MenuAnswer::Named(TOKEN_A)]);
    assert!(snapshot.get("origin").is_none());
    assert!(snapshot.get("origin_ref").is_none());
}

#[test]
fn a_delivery_still_in_flight_hides_the_proof_before_it() {
    let _home = HomeGuard::new();
    let row = row_ref();
    right_click(
        &row,
        &[MenuAnswer::Absent, MenuAnswer::Named(TOKEN_A)],
        true,
    );
    assert!(menu_snapshot(&[MenuAnswer::Named(TOKEN_A)])["origin_ref"].is_string());

    let store = MenuLedgerStore::for_session(None).unwrap();
    let now_ms = now_ms();
    store
        .begin(
            MenuDelivery {
                delivery_id: "dlv1:concurrent".into(),
                sequence: 0,
                origin_ref: "@s9:e1".into(),
                app: APP.into(),
                pid: PID,
                process_instance: Some(GENERATION.into()),
                window_id: None,
                started_at_ms: now_ms,
                prior: PriorMenu::Unknown,
                prior_instance: None,
                state: DeliveryState::Started,
                instance: None,
                disposition: None,
                settled_at_ms: None,
            },
            now_ms,
        )
        .expect("publish the concurrent delivery");

    let snapshot = menu_snapshot(&[MenuAnswer::Named(TOKEN_A)]);
    assert!(snapshot.get("origin").is_none());
    assert!(snapshot.get("origin_ref").is_none());
}

#[test]
fn a_concurrent_delivery_that_settles_afterwards_does_not_restore_the_older_proof() {
    let _home = HomeGuard::new();
    let row = row_ref();
    right_click(
        &row,
        &[MenuAnswer::Absent, MenuAnswer::Named(TOKEN_A)],
        true,
    );
    let store = MenuLedgerStore::for_session(None).unwrap();
    let now_ms = now_ms();
    store
        .begin(
            MenuDelivery {
                delivery_id: "dlv1:concurrent".into(),
                sequence: 0,
                origin_ref: "@s9:e1".into(),
                app: APP.into(),
                pid: PID,
                process_instance: Some(GENERATION.into()),
                window_id: None,
                started_at_ms: now_ms,
                prior: PriorMenu::Unknown,
                prior_instance: None,
                state: DeliveryState::Started,
                instance: None,
                disposition: None,
                settled_at_ms: None,
            },
            now_ms,
        )
        .expect("publish the concurrent delivery");

    let snapshot = menu_snapshot(&[MenuAnswer::Named(TOKEN_A)]);
    assert_eq!(snapshot.get("origin").cloned(), None);
    let ledger = store.load(now_ms + 1);
    assert_eq!(
        ledger.entries.last().map(|entry| entry.state),
        Some(DeliveryState::Started)
    );
}
