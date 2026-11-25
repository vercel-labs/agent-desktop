//! The part of a front watch that runs after the command's closure has returned,
//! beside the work its caller still owes once it has released the interaction
//! lease (a witness read, the overlays an action raised). An action can make the
//! application spawn a helper whose window maps seconds later (a settings
//! dialog run by a child process). While a process the target spawned after
//! the capture is still running and has shown no window, the watch goes on,
//! capped, and every give-back takes its own short lease instead of holding
//! the caller's.

use std::sync::{Arc, OnceLock};
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
    deadline: Deadline,
    ends_at: Instant,
}

impl FrontPending {
    /// The rest of a watch. `ends_at` is the earliest instant it may end: the
    /// settle after the dispatch was acknowledged, and not before the closure
    /// returned.
    pub(super) fn start(
        machine: FrontGuardMachine,
        shared: Arc<FrontShared>,
        deadline: Deadline,
        ends_at: Instant,
    ) -> Self {
        Self {
            machine,
            shared,
            deadline,
            ends_at,
        }
    }

    /// The same watch with a deadline of its own, for a caller whose command
    /// has used up the lease's.
    pub(crate) fn with_deadline(mut self, deadline: Deadline) -> Self {
        self.deadline = deadline;
        self
    }

    /// Runs `work`, the reads the caller owes after releasing the lease, with
    /// the watch going on beside it on its own thread. The watch ends when
    /// `work` is done and the settle has run, so it adds no time when `work`
    /// outlasts the settle. A helper the target spawned, still running with no
    /// window shown, keeps it going after that, up to a cap.
    pub(crate) fn run_around<R>(
        self,
        adapter: &dyn PlatformAdapter,
        work: impl FnOnce() -> R,
    ) -> (R, FrontResolution) {
        let done = OnceLock::new();
        std::thread::scope(|scope| {
            let watching = scope.spawn(|| self.watch(adapter, &done));
            let value = work();
            let _ = done.set(Instant::now());
            watching.thread().unpark();
            let resolution = watching.join().unwrap_or(FrontResolution {
                outcome: FrontOutcome::Off,
                pending_reason: None,
            });
            (value, resolution)
        })
    }

    /// The rest of the watch with no work beside it.
    #[cfg(test)]
    pub(crate) fn finish(self, adapter: &dyn PlatformAdapter) -> FrontResolution {
        self.run_around(adapter, || ()).1
    }

    fn watch(
        mut self,
        adapter: &dyn PlatformAdapter,
        work_done: &OnceLock<Instant>,
    ) -> FrontResolution {
        self.machine.resume();
        let mut over = false;
        let mut listed = Instant::now() - DESCENDANT_POLL;
        while !over {
            if work_done.get().is_some() && Instant::now() >= self.ends_at {
                break;
            }
            if listed.elapsed() >= DESCENDANT_POLL {
                listed = Instant::now();
                self.helper_still_running(adapter);
            }
            over = self.look(adapter).0 == Step::Done;
            if !over {
                std::thread::park_timeout(POLL);
            }
        }
        let helper_pending = !over
            && FrontOutcome::from_state(self.machine.state()) == FrontOutcome::Clean
            && self.helper_still_running(adapter);
        let reason = if helper_pending {
            self.wait_for_helper_window(adapter)
        } else {
            None
        };
        let _ = self.machine.sight(Sighting::Settled);
        FrontResolution {
            outcome: FrontOutcome::from_state(self.machine.state()),
            pending_reason: reason,
        }
    }

    fn helper_still_running(&self, adapter: &dyn PlatformAdapter) -> bool {
        let Some(root) = self.shared.root() else {
            return false;
        };
        let spawned = adapter
            .spawned_descendants(root, self.shared.captured_ms())
            .unwrap_or_default();
        let running = !spawned.is_empty();
        self.shared.set_descendants(spawned);
        running
    }

    /// One read of the front and what it asks for: a give-back under a lease
    /// of its own for an intruder. Says whether a window of a spawned helper is
    /// in view or was given back.
    fn look(&mut self, adapter: &dyn PlatformAdapter) -> (Step, bool) {
        let Ok(view) = read(adapter) else {
            return (Step::Poll, false);
        };
        self.shared.prefetch(adapter, &view);
        let mut seen = view.owners.iter().any(|pid| self.shared.owns_spawned(*pid));
        let mut step = self.machine.sight(Sighting::Front(view));
        while let Step::Return(intruder) = step {
            seen = true;
            let restored = self.give_back_under_own_lease(adapter, &intruder);
            step = self.machine.returned(restored);
        }
        (step, seen)
    }

    /// Watches until the spawned helper shows a window and the settle after it
    /// has run, until it exits without one, or until the cap. Returns why it
    /// ran out, when it did.
    fn wait_for_helper_window(&mut self, adapter: &dyn PlatformAdapter) -> Option<&'static str> {
        let ends = Instant::now() + CHILD_WATCH_CAP.min(self.deadline.remaining());
        let mut window_seen: Option<Instant> = None;
        let mut listed = Instant::now();
        loop {
            let now = Instant::now();
            if now >= ends {
                return window_seen.is_none().then_some("child_window_pending");
            }
            if now.duration_since(listed) >= DESCENDANT_POLL {
                listed = now;
                if !self.helper_still_running(adapter) {
                    return None;
                }
            }
            let (step, seen) = self.look(adapter);
            if seen {
                window_seen.get_or_insert(Instant::now());
            }
            if step == Step::Done
                || window_seen.is_some_and(|since| since.elapsed() >= AFTER_WINDOW_SETTLE)
            {
                return None;
            }
            std::thread::sleep(POLL);
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
