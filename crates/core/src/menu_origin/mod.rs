//! Provenance for the application-owned `menu` surface.
//!
//! A menu answer names the application that owns it and nothing about how it
//! came to be open, so a caller that opened a menu by right-clicking a row
//! cannot tell its own menu from one that was already there. This module closes
//! that gap with evidence the driver alone holds: the right click publishes
//! itself, reads the menu on the way in, and fixes the menu instance it
//! produced; a later menu read is attributed to that delivery only while the
//! menu on screen is the very instance it fixed.
//!
//! Nothing here is inferred from where the menu is drawn, and nothing is
//! inferred from what the menu shows: two openings look identical, so only a
//! platform element token can tell them apart.

mod instance;
mod ledger;
mod probe;
mod store;
mod witness;

pub(crate) use witness::MenuWitness;

use crate::{CommandContext, ProcessId, SnapshotSurface, snapshot::SnapshotResult};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

use ledger::MenuDelivery;
use store::MenuLedgerStore;

const DELIVERY_PREFIX: &str = "dlv1:";
const DELIVERY: &str = "right-click";
static DELIVERY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A fresh id for one delivery attempt.
///
/// It must differ between two runs of the same command in the same millisecond,
/// so it mixes the clock, a per-process counter and the process id.
pub(crate) fn new_delivery_id() -> String {
    let counter = DELIVERY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut bytes = Vec::with_capacity(28);
    bytes.extend_from_slice(&nanos.to_le_bytes());
    bytes.extend_from_slice(&counter.to_le_bytes());
    bytes.extend_from_slice(&std::process::id().to_le_bytes());
    format!("{DELIVERY_PREFIX}{:016x}", instance::digest(&bytes))
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Adds the menu's provenance to a menu-surface snapshot answer.
///
/// The answer keeps its shape when nothing can be attributed: an absent
/// `origin` is the caller's signal that the menu it read is not the instance any
/// recorded delivery fixed, and a caller that needs that proof refuses to act.
pub(crate) fn annotate(
    body: &mut Value,
    result: &SnapshotResult,
    surface: SnapshotSurface,
    context: &CommandContext,
) {
    if surface != SnapshotSurface::Menu || !result.complete {
        return;
    }
    let Some(observed) = instance::of_tree(&result.tree) else {
        return;
    };
    let Some((app, pid, process_instance)) = owner_identity(result) else {
        return;
    };
    let Ok(store) = MenuLedgerStore::for_session(context.session_id()) else {
        return;
    };
    let now_ms = now_ms();
    let ledger = store.load(now_ms);
    let Some(delivery) =
        ledger.attribution(&observed, &app, pid, process_instance.as_deref(), now_ms)
    else {
        return;
    };
    let mut origin = json!({ "surface": SnapshotSurface::Menu.as_str(), "delivery": DELIVERY });
    delivery_fields(
        &mut origin,
        delivery,
        delivery.instance.as_deref().unwrap_or_default(),
        delivery.settled_at_ms.unwrap_or(delivery.started_at_ms),
    );
    origin["proof"] = json!("fresh_after_delivery");
    body["origin_ref"] = json!(delivery.origin_ref);
    body["origin"] = origin;
}

fn owner_identity(result: &SnapshotResult) -> Option<(String, ProcessId, Option<String>)> {
    match &result.owner {
        crate::SnapshotOwner::Application(app) => {
            Some((app.name.clone(), app.pid, app.process_instance.clone()))
        }
        crate::SnapshotOwner::Window(window) => Some((
            window.app.clone(),
            window.pid,
            window.process_instance.clone(),
        )),
    }
}

/// The fields a receipt and a proven `origin` share.
pub(crate) fn delivery_fields(
    value: &mut Value,
    delivery: &MenuDelivery,
    instance: &str,
    delivered_at_ms: u64,
) {
    value["delivery_id"] = json!(delivery.delivery_id);
    value["ref_id"] = json!(delivery.origin_ref);
    value["instance"] = json!(instance);
    value["app"] = json!(delivery.app);
    value["pid"] = json!(delivery.pid.get());
    value["prior"] = json!(delivery.prior);
    value["sequence"] = json!(delivery.sequence);
    value["delivered_at_ms"] = json!(delivered_at_ms);
    if let Some(process_instance) = delivery.process_instance.as_deref() {
        value["process_instance"] = json!(process_instance);
    }
    if let Some(window_id) = delivery.window_id.as_deref() {
        value["window_id"] = json!(window_id);
    }
}

/// Rebuilds the receipt of a delivery the witness just proved.
pub(crate) fn receipt(delivery: &MenuDelivery, delivered_at_ms: u64) -> Value {
    let mut value = json!({ "delivery": DELIVERY });
    delivery_fields(
        &mut value,
        delivery,
        delivery.instance.as_deref().unwrap_or_default(),
        delivered_at_ms,
    );
    value
}

#[cfg(test)]
#[path = "menu_origin_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "menu_flow_fixture_tests.rs"]
mod flow_fixture_tests;

#[cfg(test)]
#[path = "menu_flow_tests.rs"]
mod flow_tests;

#[cfg(test)]
#[path = "menu_flow_state_tests.rs"]
mod flow_state_tests;
