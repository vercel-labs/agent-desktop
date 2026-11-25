//! The part of a front watch that runs after the command has returned and its
//! caller has released the interaction lease. An action can make the
//! application spawn a helper whose window maps seconds later (a settings
//! dialog run by a child process). While a process the target spawned after
//! the capture is still running and has shown no window, the watch goes on,
//! capped, and every give-back takes its own short lease instead of holding
//! the caller's.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::front_guard::{POLL, give_back, read};
use super::front_guard_machine::{FrontGuardMachine, Sighting, Step};
use super::front_outcome::FrontOutcome;
use super::front_shared::FrontShared;
use crate::{Deadline, adapter::PlatformAdapter};

/// The longest the watch outlives the lease for a helper that has not shown a
/// window; the command's own deadline caps it too.
const CHILD_WATCH_CAP: Duration = Duration::from_secs(10);
/// How often the descendants are listed.
const DESCENDANT_POLL: Duration = Duration::from_millis(100);
/// How long the watch goes on after the helper's window has been seen or given
/// back, for a second window or a regrab.
const AFTER_WINDOW_SETTLE: Duration = Duration::from_millis(250);
const GIVE_BACK_LEASE_MS: u64 = 2_000;

/// How the rest of a watch ended.
pub(crate) struct FrontResolution {
    pub outcome: FrontOutcome,
    /// Set when the watch ran out with a spawned process alive and no window
    /// of it seen: a window may still follow.
    pub pending_reason: Option<&'static str>,
}

pub(crate) struct FrontPending {
    machine: FrontGuardMachine,
    shared: Arc<FrontShared>,
    root: u32,
    deadline: Deadline,
}

impl FrontPending {
    /// The rest of a watch, when there is one: the adapter lists descendants
    /// and the target has a spawned process running.
    pub(super) fn start(
        adapter: &dyn PlatformAdapter,
        machine: FrontGuardMachine,
        shared: Arc<FrontShared>,
        deadline: Deadline,
    ) -> Option<Self> {
        let root = shared.target().pid?;
        let spawned = adapter.spawned_descendants(root, shared.captured_ms())?;
        if spawned.is_empty() {
            return None;
        }
        shared.set_descendants(spawned);
        Some(Self {
            machine,
            shared,
            root,
            deadline,
        })
    }

    /// Watches until the spawned process shows a window and the settle after
    /// it has run, until it exits without one, or until the cap.
    pub(crate) fn finish(mut self, adapter: &dyn PlatformAdapter) -> FrontResolution {
        self.machine.resume();
        let cap = CHILD_WATCH_CAP.min(self.deadline.remaining());
        let ends = Instant::now() + cap;
        let mut window_seen: Option<Instant> = None;
        let mut listed = Instant::now();
        let mut reason = None;
        loop {
            let now = Instant::now();
            if now >= ends {
                reason = window_seen.is_none().then_some("child_window_pending");
                break;
            }
            if now.duration_since(listed) >= DESCENDANT_POLL {
                listed = now;
                let spawned = adapter
                    .spawned_descendants(self.root, self.shared.captured_ms())
                    .unwrap_or_default();
                let none_left = spawned.is_empty();
                self.shared.set_descendants(spawned);
                if none_left {
                    break;
                }
            }
            let Ok(view) = read(adapter) else {
                std::thread::sleep(POLL);
                continue;
            };
            self.shared.prefetch(adapter, &view);
            let owned = view.owners.iter().any(|pid| self.shared.owns_spawned(*pid));
            let mut step = self.machine.sight(Sighting::Front(view));
            while let Step::Return(intruder) = step {
                window_seen.get_or_insert(Instant::now());
                let restored = self.give_back_under_own_lease(adapter, &intruder);
                step = self.machine.returned(restored);
            }
            if owned {
                window_seen.get_or_insert(Instant::now());
            }
            if step == Step::Done
                || window_seen.is_some_and(|seen| seen.elapsed() >= AFTER_WINDOW_SETTLE)
            {
                break;
            }
            std::thread::sleep(POLL);
        }
        let _ = self.machine.sight(Sighting::Settled);
        FrontResolution {
            outcome: FrontOutcome::from_state(self.machine.state()),
            pending_reason: reason,
        }
    }

    /// A give-back after the caller's lease is gone takes a short lease of its
    /// own; a lease it cannot get in time is a give-back that did not happen.
    fn give_back_under_own_lease(
        &self,
        adapter: &dyn PlatformAdapter,
        intruder: &super::front_window::FrontWindow,
    ) -> bool {
        let Ok(deadline) = Deadline::detached_after(GIVE_BACK_LEASE_MS) else {
            return false;
        };
        let Ok(lease) = adapter.acquire_interaction_lease(deadline) else {
            return false;
        };
        give_back(adapter, &lease, &self.machine, intruder)
    }
}
