//! The menu-provenance fixtures, driven through the commands a caller runs.
//!
//! Every case here starts at `right-click` and reads the answer at
//! `snapshot --surface menu`, because that is the pair the contract is for: an
//! answer that only holds up inside the ledger is not a contract a caller can
//! use.

use super::flow_fixture_tests::*;
use crate::refs::refs_test_support::HomeGuard;
use serde_json::json;

#[test]
fn the_first_open_reports_its_delivery_and_the_snapshot_echoes_it() {
    let _home = HomeGuard::new();
    let row = row_ref();

    let click = right_click(
        &row,
        &[MenuAnswer::Absent, MenuAnswer::Named(TOKEN_A)],
        true,
    );
    let receipt = &click["menu_delivery"];
    let delivery_id = receipt["delivery_id"].clone();
    assert!(delivery_id.is_string());
    assert_eq!(receipt["ref_id"], json!(row));
    assert_eq!(receipt["instance"], json!(instance_of(TOKEN_A)));
    assert_eq!(receipt["app"], json!(APP));
    assert_eq!(receipt["window_id"], json!("w-1"));
    assert_eq!(receipt["prior"], json!("none"));

    let snapshot = menu_snapshot(&[MenuAnswer::Named(TOKEN_A)]);
    assert_eq!(snapshot["origin"]["delivery_id"], delivery_id);
    assert_eq!(snapshot["origin"]["ref_id"], json!(row));
    assert_eq!(snapshot["origin"]["instance"], json!(instance_of(TOKEN_A)));
    assert_eq!(snapshot["origin"]["proof"], json!("fresh_after_delivery"));
    assert_eq!(snapshot["origin_ref"], json!(row));
}

#[test]
fn a_retry_that_renews_the_menu_reports_its_own_delivery_and_the_old_menu_stops_matching() {
    let _home = HomeGuard::new();
    let row = row_ref();
    let first = right_click(
        &row,
        &[MenuAnswer::Absent, MenuAnswer::Named(TOKEN_A)],
        true,
    );
    let first_id = first["menu_delivery"]["delivery_id"].clone();
    assert!(menu_snapshot(&[MenuAnswer::Named(TOKEN_A)])["origin"]["delivery_id"] == first_id);

    let retry = right_click(
        &row,
        &[MenuAnswer::Named(TOKEN_A), MenuAnswer::Named(TOKEN_B)],
        true,
    );
    let retry_id = retry["menu_delivery"]["delivery_id"].clone();
    assert_ne!(retry_id, first_id);
    assert_eq!(
        retry["menu_delivery"]["instance"],
        json!(instance_of(TOKEN_B))
    );
    assert_eq!(
        retry["menu_delivery"]["prior"],
        json!("open"),
        "the retry saw the old menu on the way in"
    );

    let renewed = menu_snapshot(&[MenuAnswer::Named(TOKEN_B)]);
    assert_eq!(renewed["origin"]["delivery_id"], retry_id);
    assert_eq!(renewed["origin"]["instance"], json!(instance_of(TOKEN_B)));

    let stale = menu_snapshot(&[MenuAnswer::Named(TOKEN_A)]);
    assert!(stale.get("origin").is_none());
    assert!(stale.get("origin_ref").is_none());
}

#[test]
fn a_retry_that_left_the_old_menu_in_place_reports_nothing() {
    let _home = HomeGuard::new();
    let row = row_ref();
    right_click(
        &row,
        &[MenuAnswer::Absent, MenuAnswer::Named(TOKEN_A)],
        true,
    );

    let retry = right_click(
        &row,
        &[MenuAnswer::Named(TOKEN_A), MenuAnswer::Named(TOKEN_A)],
        true,
    );

    assert!(retry.get("menu_delivery").is_none());
    let snapshot = menu_snapshot(&[MenuAnswer::Named(TOKEN_A)]);
    assert!(snapshot.get("origin").is_none());
    assert!(snapshot.get("origin_ref").is_none());
}

#[test]
fn a_menu_nobody_opened_reports_nothing() {
    let _home = HomeGuard::new();

    let snapshot = menu_snapshot(&[MenuAnswer::Named(TOKEN_A)]);

    assert!(snapshot.get("origin").is_none());
    assert!(snapshot.get("origin_ref").is_none());
}

#[test]
fn a_menu_the_platform_did_not_name_reports_nothing() {
    let _home = HomeGuard::new();
    let row = row_ref();

    let click = right_click(&row, &[MenuAnswer::Absent, MenuAnswer::Unnamed], true);
    assert!(click.get("menu_delivery").is_none());

    let snapshot = menu_snapshot(&[MenuAnswer::Unnamed]);
    assert!(snapshot.get("origin").is_none());
}

#[test]
fn a_delivery_that_did_not_land_reports_nothing() {
    let _home = HomeGuard::new();
    let row = row_ref();

    let click = right_click(&row, &[MenuAnswer::Absent], false);
    assert!(click.get("menu_delivery").is_none());

    let snapshot = menu_snapshot(&[MenuAnswer::Named(TOKEN_A)]);
    assert!(snapshot.get("origin").is_none());
}

#[test]
fn an_unreadable_menu_after_the_delivery_reports_nothing() {
    let _home = HomeGuard::new();
    let row = row_ref();

    let click = right_click(&row, &[MenuAnswer::Absent, MenuAnswer::Unreadable], true);

    assert!(click.get("menu_delivery").is_none());
}
