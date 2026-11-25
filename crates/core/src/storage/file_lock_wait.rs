use std::fs::File;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use crate::Deadline;

pub(crate) enum LockWaitFailure {
    TimedOut,
    Failed(String),
}

/// What a blocked `lock` call has produced so far. `Abandoned` is the waiter
/// that ran out of time: the blocked thread owns the file and closes it as
/// soon as the kernel grants it, so a late grant never holds the lock.
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

/// Blocks in the kernel for the lock until `deadline`. The blocking call runs on
/// a helper thread because it cannot be interrupted; the caller waits on a
/// condition variable for the grant or the deadline, whichever comes first.
pub(crate) fn wait_for_lock(file: File, deadline: Deadline) -> Result<File, LockWaitFailure> {
    let pending = Arc::new(Pending {
        state: Mutex::new(Wait::Blocked),
        changed: Condvar::new(),
    });
    let blocked = Arc::clone(&pending);
    std::thread::Builder::new()
        .name("agent-desktop-lock-wait".into())
        .spawn(move || {
            let outcome = file.lock();
            let mut state = blocked.state();
            if matches!(*state, Wait::Abandoned) {
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
            *state = Wait::Abandoned;
            return Err(LockWaitFailure::TimedOut);
        }
        state = crate::wait::deadline::wait_on(&pending.changed, state, remaining);
    }
}

#[cfg(test)]
#[path = "file_lock_wait_tests.rs"]
mod tests;
