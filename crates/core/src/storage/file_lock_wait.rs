use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::Deadline;
use crate::session::process_lease_queue::LeaseQueue;

pub(crate) enum LockWaitFailure {
    TimedOut,
    Failed(String),
}

struct PathTurns {
    queues: Mutex<BTreeMap<PathBuf, LeaseQueue>>,
    changed: Condvar,
}

static PATH_TURNS: PathTurns = PathTurns {
    queues: Mutex::new(BTreeMap::new()),
    changed: Condvar::new(),
};

impl PathTurns {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<PathBuf, LeaseQueue>> {
        self.queues
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// An in-process FIFO turn on one lock path. Held across the OS file lock's
/// lifetime and dropped only after the file closes, so a thread that releases
/// and immediately loops back queues behind waiting threads instead of barging
/// ahead of them via `try_lock`.
pub(crate) struct PathTurnGuard {
    path: PathBuf,
}

impl Drop for PathTurnGuard {
    fn drop(&mut self) {
        let mut queues = PATH_TURNS.lock();
        if let Some(queue) = queues.get_mut(&self.path) {
            queue.release();
            if queue.is_idle() {
                queues.remove(&self.path);
            }
        }
        PATH_TURNS.changed.notify_all();
    }
}

/// Reserves the calling thread's FIFO turn for `path` within `deadline`.
/// Returns the guard and whether the thread waited behind another local holder.
pub(crate) fn acquire_path_turn(
    path: &Path,
    deadline: Deadline,
) -> Result<(PathTurnGuard, bool), LockWaitFailure> {
    let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut queues = PATH_TURNS.lock();
    let ticket = queues
        .entry(key.clone())
        .or_insert_with(LeaseQueue::new)
        .enqueue();
    let mut contended = false;
    loop {
        if queues
            .get_mut(&key)
            .is_some_and(|queue| queue.try_grant(ticket))
        {
            return Ok((PathTurnGuard { path: key }, contended));
        }
        contended = true;
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            if let Some(queue) = queues.get_mut(&key) {
                queue.abandon(ticket);
                if queue.is_idle() {
                    queues.remove(&key);
                }
            }
            PATH_TURNS.changed.notify_all();
            return Err(LockWaitFailure::TimedOut);
        }
        queues = crate::wait::deadline::wait_on(&PATH_TURNS.changed, queues, remaining);
    }
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
    state: Mutex<(BTreeMap<PathBuf, usize>, BTreeMap<PathBuf, u64>)>,
    finished: Condvar,
}

static GHOSTS: Ghosts = Ghosts {
    state: Mutex::new((BTreeMap::new(), BTreeMap::new())),
    finished: Condvar::new(),
};

impl Ghosts {
    fn state(&self) -> MutexGuard<'_, (BTreeMap<PathBuf, usize>, BTreeMap<PathBuf, u64>)> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn vanished_epoch(&self, path: &Path) -> u64 {
        self.state().1.get(path).copied().unwrap_or(0)
    }

    fn appeared(&self, path: &Path) {
        *self.state().0.entry(path.to_path_buf()).or_insert(0) += 1;
    }

    fn vanished(&self, path: &Path) {
        let mut state = self.state();
        if let Some(count) = state.0.get_mut(path) {
            *count -= 1;
            if *count == 0 {
                state.0.remove(path);
            }
        }
        let epoch = state.1.entry(path.to_path_buf()).or_insert(0);
        *epoch = epoch.wrapping_add(1);
        self.finished.notify_all();
    }
}

/// How long a live acquirer lets an abandoned helper of its own process finish.
/// The kernel's grant to the helper and the helper's release are two steps a
/// few microseconds apart. A helper still blocked after this long is queued
/// behind another holder, and the acquirer queues behind it in the kernel.
const ABANDONED_HELPER_GRACE: Duration = Duration::from_millis(500);

/// Reads the current abandoned-waiter completion epoch for `path` before a
/// non-blocking lock attempt, so `settle_abandoned_waiters` detects a helper
/// that released the lock between `try_lock` and the ghost check.
pub(crate) fn abandoned_waiter_epoch(path: &Path) -> u64 {
    GHOSTS.vanished_epoch(path)
}

/// Waits, in real time, for the abandoned helpers of this process that are
/// blocked on `path` to finish, for at most the grace and the remaining budget.
/// Returns `true` when it waited and none is left, or when an abandoned helper
/// for `path` finished since `before_epoch`, so the caller tries the lock
/// again; `false` when none finished and none is left, or one is still blocked.
pub(crate) fn settle_abandoned_waiters(path: &Path, deadline: Deadline, before_epoch: u64) -> bool {
    let real_end = Instant::now() + deadline.remaining().min(ABANDONED_HELPER_GRACE);
    let mut state = GHOSTS.state();
    let mut waited = false;
    while state.0.contains_key(path) {
        let left = real_end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        waited = true;
        state = crate::wait::deadline::wait_for_event(&GHOSTS.finished, state, left);
    }
    let current_epoch = state.1.get(path).copied().unwrap_or(0);
    waited || current_epoch != before_epoch
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
