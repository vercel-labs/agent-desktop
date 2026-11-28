use std::sync::{Condvar, Mutex, MutexGuard};

use crate::session::process_lease_queue::LeaseQueue;
use crate::{AdapterError, Deadline};

static QUEUE: Mutex<LeaseQueue> = Mutex::new(LeaseQueue::new());
static CHANGED: Condvar = Condvar::new();

pub struct ProcessLeaseGuard {
    contention_count: u64,
}

fn queue() -> MutexGuard<'static, LeaseQueue> {
    QUEUE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl ProcessLeaseGuard {
    /// Takes the in-process lease in arrival order, blocking on a condition
    /// variable rather than polling, until `deadline`.
    pub fn acquire(deadline: Deadline) -> Result<Self, AdapterError> {
        let mut state = queue();
        let ticket = state.enqueue();
        let mut contention_count = 0_u64;
        loop {
            if state.try_grant(ticket) {
                if deadline.is_expired() {
                    state.release();
                    CHANGED.notify_all();
                    return Err(timeout(deadline, contention_count));
                }
                return Ok(Self { contention_count });
            }
            contention_count = 1;
            let remaining = deadline.remaining();
            if remaining.is_zero() {
                state.abandon(ticket);
                CHANGED.notify_all();
                return Err(timeout(deadline, contention_count));
            }
            state = crate::wait::deadline::wait_on(&CHANGED, state, remaining);
        }
    }

    pub fn contention_count(&self) -> u64 {
        self.contention_count
    }
}

impl Drop for ProcessLeaseGuard {
    fn drop(&mut self) {
        queue().release();
        CHANGED.notify_all();
    }
}

fn timeout(deadline: Deadline, contention_count: u64) -> AdapterError {
    crate::session::interaction_lease::interaction_lease_timeout(
        deadline,
        serde_json::json!({
            "kind": "interaction_process_lock_timeout",
            "contention_count": contention_count,
        }),
    )
}
