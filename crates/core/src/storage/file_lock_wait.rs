use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::Deadline;

pub(crate) enum LockWaitFailure {
    TimedOut,
    Failed(String),
}

/// What a blocked `lock` call has produced so far. `Abandoned` is the waiter
/// that ran out of time: the blocked thread owns the file and closes it as
/// soon as the kernel grants it, so a late grant never keeps the lock.
enum Wait {
    Blocked,
    Granted(File),
    Failed(String),
    Abandoned,
}

struct Pending {
    state: Mutex<Wait>,
    changed: Condvar,
}

impl Pending {
    fn state(&self) -> MutexGuard<'_, Wait> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The abandoned helpers of this process that are still blocked in the kernel,
/// per lock path. The kernel grants the lock to such a helper like to any
/// other waiter, and it releases the lock a moment later. A live acquirer of
/// the same path waits for these to finish instead of reading that brief hold
/// as another command holding the lock.
struct Ghosts {
    blocked: Mutex<BTreeMap<PathBuf, usize>>,
    finished: Condvar,
}

static GHOSTS: Ghosts = Ghosts {
    blocked: Mutex::new(BTreeMap::new()),
    finished: Condvar::new(),
};

impl Ghosts {
    fn counts(&self) -> MutexGuard<'_, BTreeMap<PathBuf, usize>> {
        self.blocked
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn appeared(&self, path: &Path) {
        *self.counts().entry(path.to_path_buf()).or_insert(0) += 1;
    }

    fn vanished(&self, path: &Path) {
        let mut counts = self.counts();
        if let Some(count) = counts.get_mut(path) {
            *count -= 1;
            if *count == 0 {
                counts.remove(path);
            }
        }
        self.finished.notify_all();
    }
}

/// How long a live acquirer lets an abandoned helper of its own process finish.
/// The kernel's grant to the helper and the helper's release are two steps a
/// few microseconds apart. A helper still blocked after this long is queued
/// behind another holder, and the acquirer queues behind it in the kernel.
const ABANDONED_HELPER_GRACE: Duration = Duration::from_millis(250);

/// Waits, in real time, for the abandoned helpers of this process that are
/// blocked on `path` to finish, for at most the grace and the remaining budget.
/// Returns `true` when it waited and none is left, so the caller tries the lock
/// again; `false` when there was none or one is still blocked.
pub(crate) fn settle_abandoned_waiters(path: &Path, deadline: Deadline) -> bool {
    let real_end = Instant::now() + deadline.remaining().min(ABANDONED_HELPER_GRACE);
    let mut counts = GHOSTS.counts();
    let mut waited = false;
    while counts.contains_key(path) {
        let left = real_end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        waited = true;
        counts = crate::wait::deadline::wait_for_event(&GHOSTS.finished, counts, left);
    }
    waited
}

/// Blocks in the kernel for the lock until `deadline`. The blocking call runs on
/// a helper thread because it cannot be interrupted; the caller waits on a
/// condition variable for the grant or the deadline, whichever comes first.
pub(crate) fn wait_for_lock(
    file: File,
    deadline: Deadline,
    path: &Path,
) -> Result<File, LockWaitFailure> {
    let pending = Arc::new(Pending {
        state: Mutex::new(Wait::Blocked),
        changed: Condvar::new(),
    });
    let blocked = Arc::clone(&pending);
    let helper_path = path.to_path_buf();
    std::thread::Builder::new()
        .name("agent-desktop-lock-wait".into())
        .spawn(move || {
            let outcome = file.lock();
            let mut state = blocked.state();
            if matches!(*state, Wait::Abandoned) {
                drop(file);
                drop(state);
                GHOSTS.vanished(&helper_path);
                return;
            }
            *state = match outcome {
                Ok(()) => Wait::Granted(file),
                Err(error) => Wait::Failed(error.to_string()),
            };
            blocked.changed.notify_all();
        })
        .map_err(|error| LockWaitFailure::Failed(error.to_string()))?;
    let mut state = pending.state();
    loop {
        match std::mem::replace(&mut *state, Wait::Blocked) {
            Wait::Granted(file) => return Ok(file),
            Wait::Failed(message) => return Err(LockWaitFailure::Failed(message)),
            Wait::Blocked | Wait::Abandoned => {}
        }
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            GHOSTS.appeared(path);
            *state = Wait::Abandoned;
            return Err(LockWaitFailure::TimedOut);
        }
        state = crate::wait::deadline::wait_on(&pending.changed, state, remaining);
    }
}

#[cfg(test)]
#[path = "file_lock_wait_tests.rs"]
mod tests;
