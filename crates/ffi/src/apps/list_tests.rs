use super::complete_apps;
use agent_desktop_core::{AppInventory, ErrorCode};

#[test]
fn an_inventory_that_skipped_a_record_is_refused_rather_than_listed_whole() {
    let partial = AppInventory {
        apps: Vec::new(),
        skipped: vec![serde_json::json!({ "pid": 11, "field": "application_name" })],
    };
    let error = complete_apps(partial).expect_err("a partial inventory is refused");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(error.details.unwrap()["kind"], "app_inventory_incomplete");

    let whole = AppInventory {
        apps: Vec::new(),
        skipped: Vec::new(),
    };
    assert!(complete_apps(whole).unwrap().is_empty());
}
