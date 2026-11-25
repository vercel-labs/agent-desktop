//! The deliveries that may have opened the menu a caller is looking at.
//!
//! A menu surface names only the application that owns it. What ties a menu to
//! a delivery is the driver's own record of that delivery: the element it was
//! sent to, the menu that was on screen before it, and the menu instance it
//! confirmed afterwards. A later menu read is attributed to that delivery only
//! while it is the very instance the delivery fixed.
//!
//! Each delivery is published twice: when it starts, so a concurrent reader
//! stops accepting older proofs before the screen changes, and when it settles,
//! so the proof carries what the delivery actually produced. Readers order
//! deliveries by dispatch order, never by which process finished first.

use serde::{Deserialize, Serialize};

use crate::{DeliveryDisposition, ProcessId, menu_origin::instance::MenuInstance};

const MAX_ENTRIES: usize = 32;
const PROOF_TTL_MS: u64 = 120_000;
const CLOCK_SKEW_MS: u64 = 5_000;

/// What was on screen immediately before a delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PriorMenu {
    None,
    Open,
    Unknown,
}

/// How one delivery ended, as far as menu provenance goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeliveryState {
    /// Published before the action was sent, and not settled yet.
    Started,
    /// The menu on screen after the delivery is a new instance it fixed.
    Proven,
    /// The action did not land, or landed uncertainly.
    NotDelivered,
    /// The menu could not be read after the delivery.
    ProbeFailed,
    /// No menu was on screen after the delivery.
    MenuAbsent,
    /// A menu was read but the platform named no element for it.
    InstanceUnconfirmed,
    /// The menu on screen is the one that was there before the delivery.
    InstanceUnchanged,
    /// A menu was open before the delivery and could not be named.
    PriorUnknown,
}

/// One delivery, from the moment it was announced to the moment it settled.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct MenuDelivery {
    pub delivery_id: String,
    pub sequence: u64,
    pub origin_ref: String,
    pub app: String,
    pub pid: ProcessId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    pub started_at_ms: u64,
    pub prior: PriorMenu,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_instance: Option<String>,
    pub state: DeliveryState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<DeliveryDisposition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at_ms: Option<u64>,
}

/// What a settled delivery adds to its published record.
#[derive(Debug, Clone)]
pub(crate) struct Settlement {
    pub state: DeliveryState,
    pub instance: Option<String>,
    pub prior: PriorMenu,
    pub prior_instance: Option<String>,
    pub disposition: DeliveryDisposition,
    pub settled_at_ms: u64,
}

/// Every delivery this state root saw, in dispatch order.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct MenuLedger {
    #[serde(default)]
    pub entries: Vec<MenuDelivery>,
}

impl MenuLedger {
    /// Publishes `delivery` as started, in dispatch order.
    pub(crate) fn begin(&mut self, mut delivery: MenuDelivery, now_ms: u64) -> u64 {
        self.entries
            .retain(|entry| now_ms.saturating_sub(entry.started_at_ms) <= PROOF_TTL_MS);
        delivery.sequence = self
            .entries
            .last()
            .map_or(1, |last| last.sequence.saturating_add(1));
        delivery.state = DeliveryState::Started;
        delivery.started_at_ms = now_ms;
        self.entries.push(delivery);
        if self.entries.len() > MAX_ENTRIES {
            let excess = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..excess);
        }
        self.entries.last().map_or(1, |last| last.sequence)
    }

    /// Fills in the outcome of the delivery `delivery_id` published.
    pub(crate) fn settle(&mut self, delivery_id: &str, settlement: Settlement) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.delivery_id == delivery_id)
        else {
            return false;
        };
        entry.state = settlement.state;
        entry.instance = settlement.instance;
        entry.prior = settlement.prior;
        entry.prior_instance = settlement.prior_instance;
        entry.disposition = Some(settlement.disposition);
        entry.settled_at_ms = Some(settlement.settled_at_ms);
        true
    }

    /// Drops records no reader may rely on.
    pub(crate) fn retain_valid(&mut self, now_ms: u64) {
        self.entries.retain(|entry| {
            let plausible = entry.started_at_ms <= now_ms.saturating_add(CLOCK_SKEW_MS)
                && entry
                    .settled_at_ms
                    .is_none_or(|settled| settled <= now_ms.saturating_add(CLOCK_SKEW_MS));
            let identified = entry
                .instance
                .as_deref()
                .is_none_or(crate::menu_origin::instance::is_instance_id);
            plausible && identified && !entry.delivery_id.is_empty() && !entry.origin_ref.is_empty()
        });
    }

    /// The delivery the menu `observed` belongs to, or `None`.
    ///
    /// Attribution needs the newest delivery for the application, and that
    /// delivery must have proven the very instance now on screen for the same
    /// application and process generation. Anything else — a delivery still in
    /// flight, a delivery that opened nothing, an instance that was already
    /// there, a nameless menu, a restarted process, an expired or future-dated
    /// record — proves nothing.
    pub(crate) fn attribution(
        &self,
        observed: &MenuInstance,
        app: &str,
        pid: ProcessId,
        process_instance: Option<&str>,
        now_ms: u64,
    ) -> Option<&MenuDelivery> {
        let candidate = self
            .entries
            .iter()
            .rev()
            .find(|entry| entry.app == app && entry.pid == pid)?;
        if candidate.state != DeliveryState::Proven {
            return None;
        }
        if candidate.instance.as_deref() != Some(observed.id()) {
            return None;
        }
        if candidate.process_instance.as_deref() != process_instance || process_instance.is_none() {
            return None;
        }
        if candidate.prior == PriorMenu::Unknown
            || candidate.prior_instance.as_deref() == Some(observed.id())
        {
            return None;
        }
        if !matches!(
            candidate.disposition,
            Some(DeliveryDisposition::DeliveredVerified | DeliveryDisposition::DeliveredUnverified)
        ) {
            return None;
        }
        let settled = candidate.settled_at_ms?;
        if settled > now_ms.saturating_add(CLOCK_SKEW_MS)
            || now_ms.saturating_sub(settled) > PROOF_TTL_MS
        {
            return None;
        }
        Some(candidate)
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
