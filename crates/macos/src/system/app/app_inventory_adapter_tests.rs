use agent_desktop_core::{AppInventory, Deadline, ErrorCode, ObservationOps};
use std::cell::RefCell;

thread_local! {
    static WORKSPACE_INVENTORY: RefCell<Option<AppInventory>> = const { RefCell::new(None) };
}

pub(crate) fn workspace_inventory() -> Option<AppInventory> {
    WORKSPACE_INVENTORY.with_borrow(Clone::clone)
}

struct InventoryFixture;

impl Drop for InventoryFixture {
    fn drop(&mut self) {
        WORKSPACE_INVENTORY.set(None);
    }
}

fn install_partial_inventory() -> (serde_json::Value, InventoryFixture) {
    let skipped = serde_json::json!({"pid":11,"field":"application_name"});
    WORKSPACE_INVENTORY.set(Some(AppInventory {
        apps: Vec::new(),
        skipped: vec![skipped.clone()],
    }));
    (skipped, InventoryFixture)
}

#[test]
fn adapter_rejects_silently_partial_apps_but_inventory_reports_skipped() {
    let (skipped, _fixture) = install_partial_inventory();
    let adapter = crate::MacOSAdapter::new();
    let inventory = adapter
        .query_app_inventory(
            agent_desktop_core::AppInventoryQuery::All,
            Deadline::after(1000).unwrap(),
        )
        .unwrap();
    assert!(inventory.apps.is_empty());
    assert_eq!(inventory.skipped, vec![skipped]);

    let error =
        crate::system::app::app_inventory::list_apps_complete_until(Deadline::after(1000).unwrap())
            .expect_err("a skipped record must not become a successful complete app list");
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn complete_capture_refuses_skipped_records_as_a_retryable_failure_carrying_them() {
    let (skipped, _fixture) = install_partial_inventory();

    let error = super::capture_complete_apps(Deadline::after(5000).unwrap())
        .expect_err("a skipped record must not become a complete capture");

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert!(error.is_explicitly_retryable());
    assert_eq!(error.details.unwrap()["skipped"][0], skipped);
}
