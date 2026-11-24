use super::RefStore;
use crate::{
    AppError,
    refs::{validate_snapshot_id, write_private_file},
    snapshot::{
        baseline::{BASELINE_FORMAT_VERSION, MAX_BASELINE_BYTES, SnapshotBaseline},
        since::BaselineLookup,
    },
};
use std::collections::VecDeque;
use std::io::ErrorKind;
use std::path::PathBuf;

const BASELINE_FILE: &str = "baseline.json";
const EXPIRED_SNAPSHOT_IDS_FILE: &str = "expired_snapshot_ids.json";
const MAX_EXPIRED_SNAPSHOT_IDS: usize = 512;
const MAX_EXPIRED_IDS_BYTES: u64 = 64 * 1024;

impl RefStore {
    pub(crate) fn save_new_snapshot_with_baseline(
        &self,
        refmap: &RefMapRef<'_>,
        baseline: Option<&SnapshotBaseline>,
    ) -> Result<String, AppError> {
        self.with_write_lock(|| self.save_new_snapshot_unlocked_with_baseline(refmap.0, baseline))
    }

    pub(crate) fn save_new_snapshot_detached_with_baseline(
        &self,
        refmap: &RefMapRef<'_>,
        baseline: Option<&SnapshotBaseline>,
    ) -> Result<String, AppError> {
        self.with_write_lock_detached(|| {
            self.save_new_snapshot_unlocked_with_baseline(refmap.0, baseline)
        })
    }

    fn save_new_snapshot_unlocked_with_baseline(
        &self,
        refmap: &crate::refs::RefMap,
        baseline: Option<&SnapshotBaseline>,
    ) -> Result<String, AppError> {
        let snapshot_id = crate::refs::new_snapshot_id();
        self.save_snapshot_unlocked(&snapshot_id, refmap)?;
        if let Some(baseline) = baseline {
            self.save_baseline_unlocked(&snapshot_id, baseline);
        }
        self.set_latest_unlocked(&snapshot_id)?;
        self.prune_old_snapshots_unlocked(&snapshot_id)?;
        Ok(snapshot_id)
    }

    pub(crate) fn load_baseline(&self, snapshot_id: &str) -> Result<BaselineLookup, AppError> {
        validate_snapshot_id(snapshot_id)?;
        let path = self.baseline_path(snapshot_id);
        match crate::storage::private_file::read_private_bounded(&path, MAX_BASELINE_BYTES) {
            Ok(bytes) => match serde_json::from_slice::<SnapshotBaseline>(&bytes) {
                Ok(baseline) if baseline.format_version == BASELINE_FORMAT_VERSION => {
                    Ok(BaselineLookup::Found(baseline))
                }
                Ok(_) | Err(_) => Ok(BaselineLookup::Unknown),
            },
            Err(err) if err.kind() == ErrorKind::NotFound => {
                if self.is_snapshot_expired(snapshot_id) {
                    Ok(BaselineLookup::Expired)
                } else {
                    Ok(BaselineLookup::Unknown)
                }
            }
            Err(_) => Ok(BaselineLookup::Unknown),
        }
    }

    fn save_baseline_unlocked(&self, snapshot_id: &str, baseline: &SnapshotBaseline) {
        let Some(json) = baseline.serialize_with_size_check() else {
            return;
        };
        if let Err(err) = write_private_file(&self.baseline_path(snapshot_id), json.as_bytes()) {
            tracing::warn!(snapshot_id = %snapshot_id, "failed to persist snapshot baseline: {err}");
        }
    }

    pub(super) fn record_expired_snapshots_unlocked(&self, evicted_ids: &[String]) {
        if evicted_ids.is_empty() {
            return;
        }
        let mut ids = self.read_expired_ids();
        for id in evicted_ids {
            if validate_snapshot_id(id).is_err() {
                continue;
            }
            ids.retain(|existing| existing != id);
            ids.push_back(id.clone());
            while ids.len() > MAX_EXPIRED_SNAPSHOT_IDS {
                ids.pop_front();
            }
        }
        if let Ok(json) = serde_json::to_string(&ids) {
            let _ = write_private_file(&self.expired_ids_path(), json.as_bytes());
        }
    }

    pub(super) fn remove_expired_snapshot_unlocked(&self, snapshot_id: &str) {
        let path = self.expired_ids_path();
        if !path.exists() {
            return;
        }
        let mut ids = self.read_expired_ids();
        let before = ids.len();
        ids.retain(|existing| existing != snapshot_id);
        if ids.len() != before
            && let Ok(json) = serde_json::to_string(&ids)
        {
            let _ = write_private_file(&path, json.as_bytes());
        }
    }

    fn is_snapshot_expired(&self, snapshot_id: &str) -> bool {
        self.read_expired_ids()
            .iter()
            .any(|existing| existing == snapshot_id)
    }

    fn read_expired_ids(&self) -> VecDeque<String> {
        let Ok(bytes) = crate::storage::private_file::read_private_bounded(
            &self.expired_ids_path(),
            MAX_EXPIRED_IDS_BYTES,
        ) else {
            return VecDeque::new();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    pub(crate) fn baseline_path(&self, snapshot_id: &str) -> PathBuf {
        self.snapshots_dir().join(snapshot_id).join(BASELINE_FILE)
    }

    fn expired_ids_path(&self) -> PathBuf {
        self.base_dir.join(EXPIRED_SNAPSHOT_IDS_FILE)
    }
}

pub(crate) struct RefMapRef<'a>(pub(crate) &'a crate::refs::RefMap);

impl crate::refs::RefMap {
    pub(crate) fn redact_secure_values(&mut self) {
        for entry in self.inner.values_mut() {
            if crate::tree::state::has_state(&entry.capabilities.states, crate::tree::state::SECURE)
            {
                entry.identity.value = None;
            }
        }
    }
}
