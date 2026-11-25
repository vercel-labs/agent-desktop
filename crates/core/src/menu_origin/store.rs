//! Where the menu ledger lives between driver invocations.
//!
//! The ledger is session state, not evidence a caller asked for: a ledger that
//! cannot be read or written costs provenance, never the command that touched
//! it. A read that fails yields an empty ledger, and an empty ledger attributes
//! nothing. A write that fails is retried once as a replacement so a concurrent
//! reader still sees the delivery starting instead of an older proof the
//! delivery has already invalidated; when even that write fails the delivery
//! reports no receipt.

use std::io::ErrorKind;
use std::path::PathBuf;

use crate::storage::file_lock::FileLock;
use crate::{AppError, Deadline};

use super::ledger::{DeliveryState, MenuDelivery, MenuLedger, Settlement};

const LEDGER_FILE: &str = "menu_origin.json";
const LOCK_FILE: &str = "menu_origin.lock";
const MAX_LEDGER_BYTES: u64 = 262_144;
const LOCK_TIMEOUT_MS: u64 = 2_000;

/// The ledger of one state root or one session.
pub(crate) struct MenuLedgerStore {
    dir: PathBuf,
}

impl MenuLedgerStore {
    pub(crate) fn for_session(session_id: Option<&str>) -> Result<Self, AppError> {
        let dir = match session_id {
            Some(session_id) => crate::session::session_dir(session_id)?,
            None => crate::session::agent_desktop_dir()?,
        };
        Ok(Self { dir })
    }

    fn ledger_path(&self) -> PathBuf {
        self.dir.join(LEDGER_FILE)
    }

    fn lock(&self) -> Result<FileLock, AppError> {
        crate::storage::private_file_parent::ensure_private(&self.dir).map_err(AppError::Io)?;
        FileLock::acquire(
            &self.dir.join(LOCK_FILE),
            Deadline::detached_after(LOCK_TIMEOUT_MS)?,
            "menu origin ledger lock",
        )
        .map_err(AppError::from)
    }

    /// Reads the ledger, treating an absent, unreadable or damaged file as
    /// empty.
    ///
    /// The bytes come from one private read, so permissions, file type and
    /// length are checked against the same handle that produced them. Records a
    /// reader may not rely on are dropped before they can attribute anything.
    pub(crate) fn load(&self, now_ms: u64) -> MenuLedger {
        let mut ledger = match crate::storage::private_file::read_private_bounded(
            &self.ledger_path(),
            MAX_LEDGER_BYTES,
        ) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(error) if error.kind() == ErrorKind::NotFound => MenuLedger::default(),
            Err(_) => MenuLedger::default(),
        };
        ledger.retain_valid(now_ms);
        ledger
    }

    /// Publishes a delivery that is about to be sent.
    ///
    /// A failed write is retried as a replacement ledger holding only this
    /// delivery. When both writes fail the caller gets the error and reports no
    /// receipt, so a proof the ledger never carried cannot be presented.
    pub(crate) fn begin(&self, delivery: MenuDelivery, now_ms: u64) -> Result<u64, AppError> {
        let attempt: Result<u64, AppError> = (|| {
            let lock = self.lock()?;
            let mut ledger = self.load(now_ms);
            let sequence = ledger.begin(delivery.clone(), now_ms);
            self.write(&ledger)?;
            drop(lock);
            Ok(sequence)
        })();
        attempt.or_else(|_| self.recover_with_started(&delivery, now_ms))
    }

    /// Fills in the outcome of a published delivery.
    ///
    /// `Ok(false)` means the ledger no longer holds that delivery, so nothing
    /// was written and the delivery may not be presented as proven.
    pub(crate) fn settle(
        &self,
        delivery_id: &str,
        settlement: Settlement,
    ) -> Result<Option<MenuDelivery>, AppError> {
        let lock = self.lock()?;
        let mut ledger = self.load(settlement.settled_at_ms);
        if !ledger.settle(delivery_id, settlement) {
            return Ok(None);
        }
        self.write(&ledger)?;
        let settled = ledger
            .entries
            .iter()
            .rev()
            .find(|entry| entry.delivery_id == delivery_id)
            .cloned();
        drop(lock);
        Ok(settled)
    }

    /// The write a failed `begin` falls back to: a ledger holding only this
    /// delivery, so a reader sees the new dispatch and not the proof before it.
    fn recover_with_started(&self, delivery: &MenuDelivery, now_ms: u64) -> Result<u64, AppError> {
        let lock = self.lock()?;
        let mut only = delivery.clone();
        only.sequence = 1;
        only.state = DeliveryState::Started;
        only.started_at_ms = now_ms;
        only.settled_at_ms = None;
        only.instance = None;
        let ledger = MenuLedger {
            entries: vec![only],
        };
        self.write(&ledger)?;
        drop(lock);
        Ok(1)
    }

    fn write(&self, ledger: &MenuLedger) -> Result<(), AppError> {
        let bytes = serde_json::to_vec(ledger).map_err(AppError::Json)?;
        crate::refs::write_private_file(&self.ledger_path(), &bytes)
    }
}
