//! Runs a headless command with a front guard beside it. The guard reads the
//! user's front before the command, watches on its own thread while the
//! command runs and for a short settle after it, and gives the focus and the
//! window order back when a window that is not the user's took them. The
//! first read happens before the command starts, so no intrusion can go
//! unseen for the command's whole run.
//!
//! The adapter supplies three primitives: `read_front`, `keep_below` and
//! `give_back`. Everything that decides is in [`FrontGuardMachine`].

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::front_guard_machine::{FrontGuardMachine, Owner, Sighting, Step};
use super::front_guard_state::FrontGuardState;
use super::front_outcome::FrontOutcome;
use super::front_pending::FrontPending;
use super::front_shared::FrontShared;
use super::front_target::FrontTarget;
use super::front_view::FrontView;
use crate::{
    AdapterError, Deadline, ErrorCode, InteractionLease, InteractionPolicy,
    adapter::PlatformAdapter,
};

/// How long a give-back is given to show in a read: a window manager applies
/// a raise a moment after the request returns.
const READBACK: Duration = Duration::from_millis(200);
const READBACK_POLL: Duration = Duration::from_millis(10);
/// How often the watcher reads the front. An intrusion is seen within one
/// poll plus one read, and the give-back follows at once.
pub(super) const POLL: Duration = Duration::from_millis(10);
const READ_BUDGET_MS: u64 = 500;

/// What one guarded command asks of the guard.
#[derive(Clone)]
pub(crate) struct FrontWatch {
    /// The process the command acts on, and so whose windows are intruders.
    pub target: FrontTarget,
    /// How long the guard keeps watching after the command returns.
    pub settle: Duration,
    /// Whether the watch goes on after the closure returns, beside the work
    /// the caller owes once it has released the lease (see [`FrontPending`]).
    pub after_release: bool,
}

/// Runs `command` under the guard and drops any watch that outlives it. See
/// [`guarded_deferred`].
pub(crate) fn guarded<T>(
    adapter: &dyn PlatformAdapter,
    policy: InteractionPolicy,
    lease: &InteractionLease,
    watch: FrontWatch,
    command: impl FnOnce(&DispatchMark) -> T,
) -> Result<(T, FrontOutcome), AdapterError> {
    guarded_deferred(adapter, policy, lease, watch, command)
        .map(|(value, outcome, _)| (value, outcome))
}

/// Runs `command` under the guard. A policy that lets a window take the focus
/// runs it with no guard and no reads, and so does an adapter that has no way
/// to read the front. An adapter that has one and fails to read it refuses the
/// command before it starts: a guard that cannot see the front cannot keep it.
///
/// When the watch ends with nothing seen and the target has spawned a process
/// since the capture that is still running, the guard cannot yet say that no
/// window will follow, so the third part of the answer is the rest of the
/// watch. The caller runs it with [`FrontPending::finish`] once it has
/// released the interaction lease.
pub(crate) fn guarded_deferred<T>(
    adapter: &dyn PlatformAdapter,
    policy: InteractionPolicy,
    lease: &InteractionLease,
    watch: FrontWatch,
    command: impl FnOnce(&DispatchMark) -> T,
) -> Result<(T, FrontOutcome, Option<FrontPending>), AdapterError> {
    let shared = Arc::new(FrontShared::new(watch.target.clone()));
    let mark = DispatchMark::of(Arc::clone(&shared));
    if policy.allow_focus_steal {
        return Ok((command(&mark), FrontOutcome::Off, None));
    }
    let prior = match read(adapter) {
        Ok(view) if view.focused.is_some() => view,
        Ok(_) => return Ok((command(&mark), FrontOutcome::Off, None)),
        Err(error) if error.code == ErrorCode::PlatformNotSupported => {
            return Ok((command(&mark), FrontOutcome::Off, None));
        }
        Err(_) => return Err(unreadable(policy)),
    };
    let captured_ms = unix_ms();
    shared.set_captured_ms(captured_ms);
    let owner: Owner = {
        let shared = Arc::clone(&shared);
        Box::new(move |window, is_new| shared.owns(window, is_new))
    };
    let machine = FrontGuardMachine::new(prior, owner);
    let returned = OnceLock::new();
    let watching = lease.with_deadline(lease.deadline());
    let (value, joined) = std::thread::scope(|scope| {
        let guard = scope.spawn(|| {
            let ends = Ends {
                mark: &mark,
                returned: &returned,
            };
            keep_watching(adapter, &watching, machine, &watch, &ends, &shared)
        });
        let value = command(&mark);
        let _ = returned.set(Instant::now());
        guard.thread().unpark();
        (value, guard.join().ok())
    });
    let Some((state, machine)) = joined else {
        return Ok((value, FrontOutcome::Off, None));
    };
    let outcome = FrontOutcome::from_state(state);
    let ends_at = Ends {
        mark: &mark,
        returned: &returned,
    }
    .at(watch.settle);
    let pending = (watch.after_release && !machine.is_over())
        .then(|| {
            ends_at.map(|ends_at| FrontPending::start(machine, shared, lease.deadline(), ends_at))
        })
        .flatten();
    Ok((value, outcome, pending))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

fn unreadable(policy: InteractionPolicy) -> AdapterError {
    crate::foreground_denied(
        "The window that has the focus could not be read, so a window this command opens could keep the focus, which the interaction policy does not allow",
        "focus_unreadable",
        "front-guard",
        policy,
    )
    .with_suggestion(crate::headed_mode_suggestion(
        "may take the focus from the user's window",
    ))
}

pub(super) fn read(adapter: &dyn PlatformAdapter) -> Result<FrontView, AdapterError> {
    adapter.read_front(Deadline::detached_after(READ_BUDGET_MS)?)
}

/// When the command dispatched what can open a window. The settle runs from
/// here, beside the reads the command makes after it, so the watch covers
/// dispatch through dispatch plus the settle however long those reads take.
/// A command that never marks it settles from its return.
pub(crate) struct DispatchMark {
    at: OnceLock<Instant>,
    shared: Arc<FrontShared>,
}

impl Default for DispatchMark {
    fn default() -> Self {
        Self::of(Arc::new(FrontShared::new(FrontTarget::default())))
    }
}

impl DispatchMark {
    fn of(shared: Arc<FrontShared>) -> Self {
        Self {
            at: OnceLock::new(),
            shared,
        }
    }

    pub(crate) fn dispatched(&self) {
        let _ = self.at.set(Instant::now());
    }

    /// The process the command turned out to act on, known once it returns: a
    /// launch that attached to or was forwarded to a running instance names
    /// that instance. Its windows are the target's for the rest of the watch,
    /// whatever its executable is called.
    pub(crate) fn learn_target(&self, pid: u32) {
        self.shared.learn(pid);
    }
}

struct Ends<'a> {
    mark: &'a DispatchMark,
    returned: &'a OnceLock<Instant>,
}

impl Ends<'_> {
    /// The instant the watch may end: the settle after the dispatch, or after
    /// the return when nothing was marked, and never before the return.
    fn at(&self, settle: Duration) -> Option<Instant> {
        let returned = *self.returned.get()?;
        let anchor = self.mark.at.get().copied().unwrap_or(returned);
        Some((anchor + settle).max(returned))
    }
}

fn keep_watching(
    adapter: &dyn PlatformAdapter,
    lease: &InteractionLease,
    mut machine: FrontGuardMachine,
    watch: &FrontWatch,
    ends: &Ends<'_>,
    shared: &FrontShared,
) -> (FrontGuardState, FrontGuardMachine) {
    loop {
        let end = ends.at(watch.settle);
        let settled = end.is_some_and(|end| Instant::now() >= end);
        let sighting = if settled {
            Sighting::Settled
        } else {
            match read(adapter) {
                Ok(view) => {
                    shared.prefetch(adapter, &view);
                    Sighting::Front(view)
                }
                Err(_) => Sighting::Unreadable,
            }
        };
        let mut step = machine.sight(sighting);
        while let Step::Return(intruder) = step {
            let restored = give_back(adapter, lease, &machine, &intruder);
            step = machine.returned(restored);
        }
        if step == Step::Done {
            return (machine.state(), machine);
        }
        let until_end = end.map_or(POLL, |end| end.saturating_duration_since(Instant::now()));
        std::thread::park_timeout(POLL.min(until_end));
    }
}

pub(super) fn give_back(
    adapter: &dyn PlatformAdapter,
    lease: &InteractionLease,
    machine: &FrontGuardMachine,
    intruder: &super::front_window::FrontWindow,
) -> bool {
    let prior = machine.prior();
    let (Some(focus), Some(top)) = (
        prior.focused.as_ref(),
        prior.top.as_ref().or(prior.focused.as_ref()),
    ) else {
        return false;
    };
    let _ = adapter.keep_below(intruder, top, lease);
    if adapter.give_back(focus, lease).is_err() {
        return false;
    }
    let deadline = Instant::now() + READBACK;
    loop {
        let restored = read(adapter).is_ok_and(|now| {
            now.focused.as_ref().map(|window| &window.id) == Some(&focus.id)
                && prior
                    .top
                    .as_ref()
                    .is_none_or(|top| now.top.as_ref().map(|window| &window.id) == Some(&top.id))
        });
        if restored || Instant::now() >= deadline {
            return restored;
        }
        std::thread::sleep(READBACK_POLL);
    }
}

#[cfg(test)]
#[path = "front_guard_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "front_guard_phase_tests.rs"]
mod phase_tests;

#[cfg(test)]
#[path = "front_guard_child_tests.rs"]
mod child_tests;
