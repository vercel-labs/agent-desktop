//! What a right click tells the driver about the menu it opened.
//!
//! The witness runs inside the delivery's own interaction lease. It publishes
//! the delivery before the click is sent, so a reader stops accepting the proof
//! that came before it; then it reads the menu, so it knows what was on screen
//! on the way in; and once the click settles it reads the menu again and fixes
//! the instance the delivery produced. A delivery that did not land, that
//! produced no menu, that produced a nameless menu, or whose menu is the one
//! already there fixes nothing.

use std::sync::Mutex;
use std::time::Duration;

use crate::ref_action::dispatch_witness::{DispatchOutcome, DispatchWitness};
use crate::{Deadline, DeliveryDisposition, PlatformAdapter, RefEntry, SnapshotSurface};

use super::ledger::{DeliveryState, MenuDelivery, PriorMenu, Settlement};
use super::probe::{self, MenuRead};
use super::store::MenuLedgerStore;

const PRE_READ_MS: u64 = 400;
const SETTLE_READ_MS: u64 = 1_000;

/// One delivery the witness has announced and not yet settled.
struct Pending {
    delivery_id: String,
    app: String,
    prior: PriorMenu,
    prior_instance: Option<String>,
    sequence: Option<u64>,
}

/// The right click's view of its own delivery.
pub(crate) struct MenuWitness {
    session: Option<String>,
    pending: Mutex<Option<Pending>>,
    receipt: Mutex<Option<serde_json::Value>>,
}

impl MenuWitness {
    pub(crate) fn new(session: Option<&str>) -> Self {
        Self {
            session: session.map(str::to_owned),
            pending: Mutex::new(None),
            receipt: Mutex::new(None),
        }
    }

    /// The receipt this delivery earned, when it proved a menu instance.
    pub(crate) fn receipt(&self) -> Option<serde_json::Value> {
        self.receipt.lock().ok()?.clone()
    }

    fn store(&self) -> Option<MenuLedgerStore> {
        MenuLedgerStore::for_session(self.session.as_deref()).ok()
    }

    fn begin(
        &self,
        ref_id: &str,
        entry: &RefEntry,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    ) -> Option<Pending> {
        if adapter
            .check_surface_support(SnapshotSurface::Menu)
            .is_err()
        {
            return None;
        }
        let app = entry.source.source_app.clone()?;
        let store = self.store()?;
        let now_ms = super::now_ms();
        let delivery_id = super::new_delivery_id();
        let record = MenuDelivery {
            delivery_id: delivery_id.clone(),
            sequence: 0,
            origin_ref: ref_id.to_owned(),
            app: app.clone(),
            pid: entry.process.pid,
            process_instance: entry.process.process_instance.clone(),
            window_id: entry.source.source_window_id.clone(),
            started_at_ms: now_ms,
            prior: PriorMenu::Unknown,
            prior_instance: None,
            state: DeliveryState::Started,
            instance: None,
            disposition: None,
            settled_at_ms: None,
        };
        let sequence = store.begin(record, now_ms).ok();
        let (prior, prior_instance) = if sequence.is_some() {
            prior_menu(probe::observe(
                adapter,
                &app,
                deadline.capped(Duration::from_millis(PRE_READ_MS)),
            ))
        } else {
            (PriorMenu::Unknown, None)
        };
        Some(Pending {
            delivery_id,
            app,
            prior,
            prior_instance,
            sequence,
        })
    }

    fn settle(
        &self,
        pending: Pending,
        outcome: DispatchOutcome<'_>,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    ) {
        let disposition = match &outcome {
            DispatchOutcome::Delivered(result) => result.disposition().delivery(),
            DispatchOutcome::Failed(error) => error.disposition.delivery(),
        };
        let delivered = matches!(
            disposition,
            DeliveryDisposition::DeliveredVerified | DeliveryDisposition::DeliveredUnverified
        );
        let (state, instance) = if !delivered || pending.sequence.is_none() {
            (DeliveryState::NotDelivered, None)
        } else if pending.prior == PriorMenu::Unknown {
            (DeliveryState::PriorUnknown, None)
        } else {
            after_delivery(
                adapter,
                &pending.app,
                pending.prior_instance.as_deref(),
                deadline.capped(Duration::from_millis(SETTLE_READ_MS)),
            )
        };
        let settled_at_ms = super::now_ms();
        let settlement = Settlement {
            state,
            instance,
            prior: pending.prior,
            prior_instance: pending.prior_instance,
            disposition,
            settled_at_ms,
        };
        let Some(store) = self.store() else {
            return;
        };
        let Ok(Some(settled)) = store.settle(&pending.delivery_id, settlement) else {
            return;
        };
        if settled.state != DeliveryState::Proven {
            return;
        }
        if let Ok(mut slot) = self.receipt.lock() {
            *slot = Some(super::receipt(&settled, settled_at_ms));
        }
    }
}

fn prior_menu(read: MenuRead) -> (PriorMenu, Option<String>) {
    match read {
        MenuRead::Absent => (PriorMenu::None, None),
        MenuRead::Observed(menu) => (PriorMenu::Open, Some(menu.id().to_owned())),
        MenuRead::Unidentified | MenuRead::Unknown => (PriorMenu::Unknown, None),
    }
}

fn after_delivery(
    adapter: &dyn PlatformAdapter,
    app: &str,
    prior_instance: Option<&str>,
    deadline: Deadline,
) -> (DeliveryState, Option<String>) {
    match probe::observe(adapter, app, deadline) {
        MenuRead::Absent => (DeliveryState::MenuAbsent, None),
        MenuRead::Unidentified => (DeliveryState::InstanceUnconfirmed, None),
        MenuRead::Unknown => (DeliveryState::ProbeFailed, None),
        MenuRead::Observed(menu) if prior_instance == Some(menu.id()) => {
            (DeliveryState::InstanceUnchanged, None)
        }
        MenuRead::Observed(menu) => (DeliveryState::Proven, Some(menu.id().to_owned())),
    }
}

impl DispatchWitness for MenuWitness {
    fn started(
        &self,
        ref_id: &str,
        entry: &RefEntry,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    ) {
        let pending = self.begin(ref_id, entry, adapter, deadline);
        if let Ok(mut slot) = self.pending.lock() {
            *slot = pending;
        }
    }

    fn settled(
        &self,
        _ref_id: &str,
        _entry: &RefEntry,
        outcome: DispatchOutcome<'_>,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    ) {
        let pending = self.pending.lock().ok().and_then(|mut slot| slot.take());
        if let Some(pending) = pending {
            self.settle(pending, outcome, adapter, deadline);
        }
    }
}
