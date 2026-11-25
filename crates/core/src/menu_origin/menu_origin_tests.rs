use super::*;
use crate::menu_origin::ledger::{DeliveryState, PriorMenu, Settlement};
use crate::refs::refs_test_support::HomeGuard;
use crate::{DeliveryDisposition, NodeIdentity, NodePresentation, Rect, SnapshotOwner};

const APP: &str = "Fixture";
const PID: ProcessId = ProcessId::new(42);
const GENERATION: &str = "generation-1";
const TOKEN_A: &str = "axtok1:aa";
const TOKEN_B: &str = "axtok1:bb";

fn menu(name: &str, items: &[&str], token: Option<&str>) -> crate::AccessibilityNode {
    crate::AccessibilityNode {
        ref_id: None,
        role: "menu".into(),
        identity: NodeIdentity {
            retained_object: token.map(str::to_owned),
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: NodePresentation {
            bounds: Some(Rect {
                x: 100.0,
                y: 200.0,
                width: 240.0,
                height: 120.0,
            }),
            ..Default::default()
        },
        children_count: Some(items.len() as u32),
        subtree_truncated: false,
        children: items
            .iter()
            .map(|item| crate::AccessibilityNode {
                ref_id: None,
                role: "menuitem".into(),
                identity: NodeIdentity {
                    name: Some((*item).into()),
                    ..Default::default()
                },
                presentation: Default::default(),
                children_count: None,
                subtree_truncated: false,
                children: Vec::new(),
            })
            .collect(),
    }
}

fn result(tree: crate::AccessibilityNode, complete: bool) -> SnapshotResult {
    SnapshotResult {
        tree,
        refmap: crate::refs::RefMap::new(),
        owner: SnapshotOwner::Application(crate::AppInfo {
            name: APP.into(),
            pid: PID,
            bundle_id: None,
            process_instance: Some(GENERATION.into()),
            presentation: None,
        }),
        snapshot_id: Some("@s3".into()),
        complete,
        nodes_observed: 4,
        identity_unproven: None,
    }
}

fn record(
    store: &MenuLedgerStore,
    delivery_id: &str,
    state: DeliveryState,
    token: Option<&str>,
    prior: PriorMenu,
    prior_token: Option<&str>,
) {
    let now_ms = now_ms();
    let record = MenuDelivery {
        delivery_id: delivery_id.into(),
        sequence: 0,
        origin_ref: "@s2:e9".into(),
        app: APP.into(),
        pid: PID,
        process_instance: Some(GENERATION.into()),
        window_id: Some("w-2".into()),
        started_at_ms: now_ms,
        prior: PriorMenu::Unknown,
        prior_instance: None,
        state: DeliveryState::Started,
        instance: None,
        disposition: None,
        settled_at_ms: None,
    };
    store.begin(record, now_ms).expect("publish the delivery");
    store
        .settle(
            delivery_id,
            Settlement {
                state,
                instance: token.map(instance_id),
                prior,
                prior_instance: prior_token.map(instance_id),
                disposition: DeliveryDisposition::DeliveredUnverified,
                settled_at_ms: now_ms,
            },
        )
        .expect("settle the delivery");
}

fn instance_id(token: &str) -> String {
    instance::of_tree(&menu("context", &[], Some(token)))
        .expect("a token names an instance")
        .id()
        .to_owned()
}

#[test]
fn a_menu_the_driver_opened_reports_the_ref_the_delivery_and_the_instance() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(
        &store,
        "dlv1:1",
        DeliveryState::Proven,
        Some(TOKEN_A),
        PriorMenu::None,
        None,
    );

    let mut body = json!({ "app": APP, "complete": true });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], Some(TOKEN_A)), true),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );

    assert_eq!(body["origin_ref"], json!("@s2:e9"));
    assert_eq!(body["origin"]["ref_id"], json!("@s2:e9"));
    assert_eq!(body["origin"]["delivery_id"], json!("dlv1:1"));
    assert_eq!(body["origin"]["instance"], json!(instance_id(TOKEN_A)));
    assert_eq!(body["origin"]["proof"], json!("fresh_after_delivery"));
    assert_eq!(body["origin"]["app"], json!(APP));
    assert_eq!(body["origin"]["process_instance"], json!(GENERATION));
    assert_eq!(body["origin"]["window_id"], json!("w-2"));
    assert_eq!(body["origin"]["surface"], json!("menu"));
}

#[test]
fn review_unrelated_replacement_has_no_origin() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(
        &store,
        "dlv1:1",
        DeliveryState::Proven,
        Some(TOKEN_A),
        PriorMenu::None,
        None,
    );
    let context = CommandContext::default();

    let mut opened = json!({});
    annotate(
        &mut opened,
        &result(menu("context", &["Rename"], Some(TOKEN_A)), true),
        SnapshotSurface::Menu,
        &context,
    );
    assert_eq!(opened["origin_ref"], json!("@s2:e9"));

    let mut replacement = json!({});
    annotate(
        &mut replacement,
        &result(menu("context", &["Rename"], Some(TOKEN_B)), true),
        SnapshotSurface::Menu,
        &context,
    );
    assert_eq!(replacement.get("origin").cloned(), None);
    assert_eq!(replacement.get("origin_ref").cloned(), None);
}

#[test]
fn review_changed_menu_items_do_not_prove_a_new_opening() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(
        &store,
        "dlv1:1",
        DeliveryState::Proven,
        Some(TOKEN_A),
        PriorMenu::None,
        None,
    );

    let mut response = json!({});
    annotate(
        &mut response,
        &result(menu("context", &["Rename", "Move to Trash"], None), true),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );
    assert_eq!(response.get("origin").cloned(), None);
    assert_eq!(response.get("origin_ref").cloned(), None);
}

#[test]
fn a_menu_read_without_a_recorded_delivery_names_no_origin() {
    let _home = HomeGuard::new();

    let mut body = json!({ "app": APP, "complete": true });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], Some(TOKEN_A)), true),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );

    assert!(body.get("origin_ref").is_none());
    assert!(body.get("origin").is_none());
}

#[test]
fn a_menu_that_outlived_the_delivery_under_it_names_no_origin() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(
        &store,
        "dlv1:1",
        DeliveryState::InstanceUnchanged,
        None,
        PriorMenu::Open,
        Some(TOKEN_A),
    );

    let mut body = json!({ "app": APP, "complete": true });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], Some(TOKEN_A)), true),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );

    assert!(body.get("origin_ref").is_none());
}

#[test]
fn only_a_menu_surface_carries_an_origin() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(
        &store,
        "dlv1:1",
        DeliveryState::Proven,
        Some(TOKEN_A),
        PriorMenu::None,
        None,
    );

    let mut body = json!({ "app": APP, "complete": true });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], Some(TOKEN_A)), true),
        SnapshotSurface::Window,
        &CommandContext::default(),
    );

    assert!(body.get("origin").is_none());
}

#[test]
fn an_incomplete_menu_read_carries_no_origin() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(
        &store,
        "dlv1:1",
        DeliveryState::Proven,
        Some(TOKEN_A),
        PriorMenu::None,
        None,
    );

    let mut body = json!({ "app": APP, "complete": false });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], Some(TOKEN_A)), false),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );

    assert!(body.get("origin").is_none());
}

#[test]
fn a_platform_element_token_identifies_the_menu_appearance() {
    let first = instance::of_tree(&menu("context", &["Rename"], Some(TOKEN_A))).expect("token");
    let second = instance::of_tree(&menu("context", &["Rename"], Some(TOKEN_A))).expect("token");
    let reopened = instance::of_tree(&menu("context", &["Rename"], Some(TOKEN_B))).expect("token");

    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), reopened.id());
    assert!(instance::is_instance_id(first.id()));
}

#[test]
fn a_menu_without_a_token_has_no_instance() {
    assert!(instance::of_tree(&menu("context", &["Rename"], None)).is_none());
}

#[test]
fn the_ledger_survives_the_process_that_recorded_it() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(
        &store,
        "dlv1:1",
        DeliveryState::Proven,
        Some(TOKEN_A),
        PriorMenu::None,
        None,
    );
    record(
        &store,
        "dlv1:2",
        DeliveryState::Proven,
        Some(TOKEN_B),
        PriorMenu::Open,
        Some(TOKEN_A),
    );

    let reopened = MenuLedgerStore::for_session(None).unwrap().load(now_ms());

    assert_eq!(reopened.entries.len(), 2);
    assert_eq!(reopened.entries[1].delivery_id, "dlv1:2");
    assert_eq!(reopened.entries[1].prior, PriorMenu::Open);
    assert_eq!(reopened.entries[1].sequence, 2);
}

#[test]
fn a_damaged_ledger_attributes_nothing_and_is_replaced() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    let path = crate::session::agent_desktop_dir()
        .unwrap()
        .join("menu_origin.json");
    crate::storage::private_file_parent::ensure_private(path.parent().unwrap()).unwrap();
    crate::refs::write_private_file(&path, b"{not json").unwrap();

    assert!(store.load(now_ms()).entries.is_empty());

    record(
        &store,
        "dlv1:1",
        DeliveryState::Proven,
        Some(TOKEN_A),
        PriorMenu::None,
        None,
    );
    assert_eq!(store.load(now_ms()).entries.len(), 1);
}

#[test]
fn a_ledger_holding_a_forged_proof_is_refused() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    let forged = json!({
        "entries": [{
            "delivery_id": "dlv1:forged",
            "sequence": 1,
            "origin_ref": "@s9:e1",
            "app": APP,
            "pid": 42,
            "process_instance": GENERATION,
            "started_at_ms": now_ms(),
            "prior": "none",
            "state": "proven",
            "instance": "not-an-instance",
            "disposition": "delivered_unverified",
            "settled_at_ms": now_ms()
        }]
    });
    let path = crate::session::agent_desktop_dir()
        .unwrap()
        .join("menu_origin.json");
    crate::storage::private_file_parent::ensure_private(path.parent().unwrap()).unwrap();
    crate::refs::write_private_file(&path, forged.to_string().as_bytes()).unwrap();

    assert!(store.load(now_ms()).entries.is_empty());
}
