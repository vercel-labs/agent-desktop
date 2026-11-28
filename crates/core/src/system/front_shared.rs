use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use super::front_process::FrontProcess;
use super::front_target::FrontTarget;
use super::front_view::FrontView;
use super::front_window::FrontWindow;
use crate::adapter::PlatformAdapter;

/// What the guard learns about the command's target while it runs, shared by
/// the watcher thread and the extension that follows the lease release: the
/// processes it has described, the process the command turned out to act on,
/// and the descendants the target has spawned since the capture.
pub(crate) struct FrontShared {
    target: FrontTarget,
    captured_ms: AtomicU64,
    processes: Mutex<HashMap<u32, Option<FrontProcess>>>,
    learned: OnceLock<u32>,
    descendants: Mutex<Vec<u32>>,
}

impl FrontShared {
    pub(crate) fn new(target: FrontTarget) -> Self {
        Self {
            target,
            captured_ms: AtomicU64::new(0),
            processes: Mutex::new(HashMap::new()),
            learned: OnceLock::new(),
            descendants: Mutex::new(Vec::new()),
        }
    }

    /// The process the command acts on: the one it named, or the one it
    /// turned out to launch.
    pub(crate) fn root(&self) -> Option<u32> {
        self.target.pid.or_else(|| self.learned.get().copied())
    }

    pub(crate) fn captured_ms(&self) -> u64 {
        self.captured_ms.load(Ordering::Relaxed)
    }

    pub(crate) fn set_captured_ms(&self, ms: u64) {
        self.captured_ms.store(ms, Ordering::Relaxed);
    }

    pub(crate) fn learn(&self, pid: u32) {
        let _ = self.learned.set(pid);
    }

    pub(crate) fn set_descendants(&self, pids: Vec<u32>) {
        if let Ok(mut known) = self.descendants.lock() {
            *known = pids;
        }
    }

    pub(crate) fn owns_spawned(&self, pid: u32) -> bool {
        self.is_descendant(pid)
    }

    fn is_descendant(&self, pid: u32) -> bool {
        self.descendants
            .lock()
            .is_ok_and(|known| known.contains(&pid))
    }

    /// Whether `window` is the target's: the process the command named or
    /// learned, a process the target spawned after the capture, or what
    /// `FrontTarget::owns` says from the process's executable and start.
    /// Reads the facts of the processes that own the front and the top window,
    /// once each, so a later `owns` needs no adapter.
    pub(crate) fn prefetch(&self, adapter: &dyn PlatformAdapter, view: &FrontView) {
        for window in [view.focused.as_ref(), view.top.as_ref()]
            .into_iter()
            .flatten()
        {
            if let Ok(mut known) = self.processes.lock() {
                known
                    .entry(window.pid)
                    .or_insert_with(|| adapter.front_process(window.pid));
            }
        }
    }

    pub(crate) fn owns(&self, window: &FrontWindow, is_new: bool) -> bool {
        self.learned.get() == Some(&window.pid)
            || self.is_descendant(window.pid)
            || self.target.owns(
                window,
                is_new,
                || {
                    self.processes
                        .lock()
                        .map_or(None, |known| known.get(&window.pid).cloned().flatten())
                },
                self.captured_ms(),
            )
    }
}
