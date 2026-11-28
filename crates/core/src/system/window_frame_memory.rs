use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::PathBuf;

use crate::{AdapterError, Deadline, FileLock, system::window_frame_record::WindowFrameRecord};

const FILE_NAME: &str = "window_frames.json";
const LOCK_NAME: &str = "window_frames.lock";
const MAX_BYTES: u64 = 256 * 1024;
const LOCK_TIMEOUT_MS: u64 = 2_000;

type Records = BTreeMap<String, WindowFrameRecord>;

/// The frame a window had before a maximize moved it, kept in the state root
/// so a later `restore` process can put it back. One record per window key;
/// writing drops the records of processes that are gone.
#[derive(Debug, Clone)]
pub struct WindowFrameMemory {
    root: PathBuf,
}

impl WindowFrameMemory {
    pub fn open() -> Result<Self, AdapterError> {
        let root = crate::storage::state_root::resolve_configured_state_root()
            .map_err(|error| AdapterError::internal(error.to_string()))?;
        Ok(Self { root })
    }

    #[cfg(test)]
    pub(crate) fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn recall(&self, key: &str) -> Result<Option<WindowFrameRecord>, AdapterError> {
        Ok(self.read()?.remove(key))
    }

    pub fn remember(
        &self,
        key: &str,
        record: WindowFrameRecord,
        process_is_live: impl Fn(&WindowFrameRecord) -> bool,
    ) -> Result<(), AdapterError> {
        self.update(|records| {
            records.retain(|_, kept| process_is_live(kept));
            records.insert(key.to_string(), record);
        })
    }

    pub fn forget(&self, key: &str) -> Result<(), AdapterError> {
        self.update(|records| {
            records.remove(key);
        })
    }

    fn update(&self, change: impl FnOnce(&mut Records)) -> Result<(), AdapterError> {
        let deadline = Deadline::detached_after(LOCK_TIMEOUT_MS)?;
        let _lock = FileLock::acquire(&self.root.join(LOCK_NAME), deadline, "window frame lock")?;
        let mut records = self.read()?;
        change(&mut records);
        let json = serde_json::to_vec(&records)
            .map_err(|error| AdapterError::internal(error.to_string()))?;
        crate::storage::private_file::write_atomic(&self.root.join(FILE_NAME), &json)
            .map_err(|error| AdapterError::internal(error.to_string()))
    }

    fn read(&self) -> Result<Records, AdapterError> {
        let path = self.root.join(FILE_NAME);
        match crate::storage::private_file::read_private_bounded(&path, MAX_BYTES) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Records::new()),
            Err(error) => Err(AdapterError::internal(error.to_string())),
        }
    }
}

#[cfg(test)]
#[path = "window_frame_memory_tests.rs"]
mod tests;
