use super::*;
use crate::ProcessId;
use crate::menu_origin::instance::{self, MenuInstance};

const APP: &str = "Fixture";
const PID: ProcessId = ProcessId::new(42);
const GENERATION: Option<&str> = Some("generation-1");
const NOW: u64 = 1_000_000;

fn menu_instance(token: &str) -> MenuInstance {
    instance::of_tree(&crate::AccessibilityNode {
        ref_id: None,
        role: "menu".into(),
        identity: crate::NodeIdentity {
            retained_object: Some(token.into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: Vec::new(),
    })
    .expect("a token names an instance")
}

fn started(delivery_id: &str) -> MenuDelivery {
    MenuDelivery {
        delivery_id: delivery_id.into(),
        sequence: 0,
        origin_ref: "@s1:e7".into(),
        app: APP.into(),
        pid: PID,
        process_instance: GENERATION.map(str::to_owned),
        window_id: Some("w-2".into()),
        started_at_ms: NOW,
        prior: PriorMenu::Unknown,
        prior_instance: None,
        state: DeliveryState::Started,
        instance: None,
        disposition: None,
        settled_at_ms: None,
    }
}

fn settlement(
    state: DeliveryState,
    instance: Option<&MenuInstance>,
    prior: PriorMenu,
    prior_instance: Option<&MenuInstance>,
) -> Settlement {
    Settlement {
        state,
        instance: instance.map(|menu| menu.id().to_owned()),
        prior,
        prior_instance: prior_instance.map(|menu| menu.id().to_owned()),
        disposition: DeliveryDisposition::DeliveredUnverified,
        settled_at_ms: NOW,
    }
}

fn prove(ledger: &mut MenuLedger, delivery_id: &str, menu: &MenuInstance) {
    ledger.begin(started(delivery_id), NOW);
    ledger.settle(
        delivery_id,
        settlement(DeliveryState::Proven, Some(menu), PriorMenu::None, None),
    );
}

fn attribute<'a>(ledger: &'a MenuLedger, menu: &MenuInstance) -> Option<&'a MenuDelivery> {
    ledger.attribution(menu, APP, PID, GENERATION, NOW)
}

#[test]
fn a_proven_delivery_attributes_the_menu_it_fixed() {
    let menu = menu_instance("axtok1:aa");
    let mut ledger = MenuLedger::default();
    prove(&mut ledger, "dlv1:1", &menu);

    let proof = attribute(&ledger, &menu).expect("the delivery fixed this menu");

    assert_eq!(proof.delivery_id, "dlv1:1");
    assert_eq!(proof.origin_ref, "@s1:e7");
}

#[test]
fn a_menu_the_delivery_did_not_fix_attributes_nothing() {
    let opened = menu_instance("axtok1:aa");
    let replaced = menu_instance("axtok1:bb");
    let mut ledger = MenuLedger::default();
    prove(&mut ledger, "dlv1:1", &opened);

    assert!(attribute(&ledger, &replaced).is_none());
}

#[test]
fn an_in_flight_delivery_hides_the_proof_that_came_before_it() {
    let menu = menu_instance("axtok1:aa");
    let mut ledger = MenuLedger::default();
    prove(&mut ledger, "dlv1:1", &menu);

    ledger.begin(started("dlv1:2"), NOW);

    assert!(attribute(&ledger, &menu).is_none());
}

#[test]
fn a_settlement_that_arrives_after_a_newer_start_cannot_revive_a_proof() {
    let menu = menu_instance("axtok1:aa");
    let mut ledger = MenuLedger::default();
    ledger.begin(started("dlv1:1"), NOW);
    ledger.begin(started("dlv1:2"), NOW);
    ledger.settle(
        "dlv1:1",
        settlement(DeliveryState::Proven, Some(&menu), PriorMenu::None, None),
    );

    assert!(attribute(&ledger, &menu).is_none());
}

#[test]
fn a_menu_that_did_not_renew_attributes_nothing() {
    let menu = menu_instance("axtok1:aa");
    let mut ledger = MenuLedger::default();
    ledger.begin(started("dlv1:1"), NOW);
    ledger.settle(
        "dlv1:1",
        settlement(
            DeliveryState::InstanceUnchanged,
            None,
            PriorMenu::Open,
            Some(&menu),
        ),
    );

    assert!(attribute(&ledger, &menu).is_none());
}

#[test]
fn a_restarted_application_generation_attributes_nothing() {
    let menu = menu_instance("axtok1:aa");
    let mut ledger = MenuLedger::default();
    prove(&mut ledger, "dlv1:1", &menu);

    assert!(
        ledger
            .attribution(&menu, APP, PID, Some("generation-2"), NOW)
            .is_none()
    );
    assert!(ledger.attribution(&menu, APP, PID, None, NOW).is_none());
    assert!(
        ledger
            .attribution(&menu, "Other", PID, GENERATION, NOW)
            .is_none()
    );
    assert!(
        ledger
            .attribution(&menu, APP, ProcessId::new(7), GENERATION, NOW)
            .is_none()
    );
}

#[test]
fn an_expired_or_future_dated_proof_attributes_nothing() {
    let menu = menu_instance("axtok1:aa");
    let mut ledger = MenuLedger::default();
    prove(&mut ledger, "dlv1:1", &menu);

    assert!(
        ledger
            .attribution(&menu, APP, PID, GENERATION, NOW + PROOF_TTL_MS)
            .is_some()
    );
    assert!(
        ledger
            .attribution(&menu, APP, PID, GENERATION, NOW + PROOF_TTL_MS + 1)
            .is_none()
    );

    let mut future = MenuLedger::default();
    future.begin(started("dlv1:9"), NOW);
    let mut late = settlement(DeliveryState::Proven, Some(&menu), PriorMenu::None, None);
    late.settled_at_ms = NOW + CLOCK_SKEW_MS + 1;
    future.settle("dlv1:9", late);
    assert!(attribute(&future, &menu).is_none());
}

#[test]
fn a_delivery_that_did_not_land_attributes_nothing() {
    let menu = menu_instance("axtok1:aa");
    let mut ledger = MenuLedger::default();
    ledger.begin(started("dlv1:1"), NOW);
    let mut refused = settlement(DeliveryState::NotDelivered, None, PriorMenu::None, None);
    refused.disposition = DeliveryDisposition::DeliveryUncertain;
    ledger.settle("dlv1:1", refused);

    assert!(attribute(&ledger, &menu).is_none());
}

#[test]
fn records_a_reader_cannot_trust_are_dropped() {
    let mut ledger = MenuLedger::default();
    ledger.begin(started("dlv1:1"), NOW);
    let mut forged = started("dlv1:2");
    forged.instance = Some("mnu1:not-a-digest".into());
    forged.state = DeliveryState::Proven;
    ledger.entries.push(forged);

    ledger.retain_valid(NOW);

    assert_eq!(ledger.entries.len(), 1);
    assert_eq!(ledger.entries[0].delivery_id, "dlv1:1");
}

#[test]
fn the_ledger_orders_its_entries_and_keeps_itself_bounded() {
    let mut ledger = MenuLedger::default();
    for index in 0..(MAX_ENTRIES + 8) {
        ledger.begin(started(&format!("dlv1:{index}")), NOW);
    }

    assert_eq!(ledger.entries.len(), MAX_ENTRIES);
    let sequences: Vec<u64> = ledger.entries.iter().map(|entry| entry.sequence).collect();
    let mut sorted = sequences.clone();
    sorted.sort_unstable();
    assert_eq!(sequences, sorted);
    assert_eq!(sequences.last().copied(), Some((MAX_ENTRIES + 8) as u64));
}

#[test]
fn only_a_settlement_that_names_its_own_delivery_lands() {
    let mut ledger = MenuLedger::default();
    ledger.begin(started("dlv1:1"), NOW);

    assert!(!ledger.settle(
        "dlv1:2",
        settlement(DeliveryState::MenuAbsent, None, PriorMenu::None, None)
    ));
    assert_eq!(ledger.entries[0].state, DeliveryState::Started);
}
