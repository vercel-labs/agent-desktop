//! Runs a headless command with a front guard beside it. The guard reads the
//! user's front before the command, watches on its own thread while the
//! command runs and for a short settle after it, and gives the focus and the
//! window order back when a window that is not the user's took them. The
//! first read happens before the command starts, so no intrusion can go
//! unseen for the command's whole run.
//!
//! The adapter supplies three primitives: `read_front`, `keep_below` and
//! `give_back`. Everything that decides is in [`FrontGuardMachine`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::front_guard_machine::{FrontGuardMachine, Sighting, Step};
use super::front_guard_state::FrontGuardState;
use super::front_outcome::FrontOutcome;
use super::front_view::FrontView;
use crate::{
    AdapterError, Deadline, InteractionLease, InteractionPolicy, adapter::PlatformAdapter,
};

const POLL: Duration = Duration::from_millis(20);
const READ_BUDGET_MS: u64 = 500;

/// What one guarded command asks of the guard.
#[derive(Clone, Copy)]
pub(crate) struct FrontWatch {
    /// The process the command targets, when its existing windows count as
    /// intruders too.
    pub owned_pid: Option<u32>,
    /// How long the guard keeps watching after the command returns.
    pub settle: Duration,
}

/// Runs `command`. A policy that lets a window take the focus runs it with no
/// guard and no reads.
pub(crate) fn guarded<T>(
    adapter: &dyn PlatformAdapter,
    policy: InteractionPolicy,
    lease: &InteractionLease,
    watch: FrontWatch,
    command: impl FnOnce() -> T,
) -> (T, FrontOutcome) {
    if policy.allow_focus_steal {
        return (command(), FrontOutcome::Off);
    }
    let Some(prior) = capture(adapter) else {
        return (command(), FrontOutcome::Off);
    };
    let machine = FrontGuardMachine::new(prior, watch.owned_pid);
    let finished = AtomicBool::new(false);
    let watching = lease.with_deadline(lease.deadline());
    std::thread::scope(|scope| {
        let guard = scope.spawn(|| keep_watching(adapter, &watching, machine, watch, &finished));
        let value = command();
        finished.store(true, Ordering::Release);
        let outcome = guard
            .join()
            .map_or(FrontOutcome::Off, FrontOutcome::from_state);
        (value, outcome)
    })
}

fn read(adapter: &dyn PlatformAdapter) -> Result<FrontView, AdapterError> {
    adapter.read_front(Deadline::detached_after(READ_BUDGET_MS)?)
}

fn capture(adapter: &dyn PlatformAdapter) -> Option<FrontView> {
    let view = read(adapter).ok()?;
    view.focused.is_some().then_some(view)
}

fn keep_watching(
    adapter: &dyn PlatformAdapter,
    lease: &InteractionLease,
    mut machine: FrontGuardMachine,
    watch: FrontWatch,
    finished: &AtomicBool,
) -> FrontGuardState {
    let mut settling_since: Option<Instant> = None;
    loop {
        if finished.load(Ordering::Acquire) {
            settling_since.get_or_insert_with(Instant::now);
        }
        let settled = settling_since.is_some_and(|since| since.elapsed() >= watch.settle);
        let sighting = if settled {
            Sighting::Settled
        } else {
            match read(adapter) {
                Ok(view) => Sighting::Front(view),
                Err(_) => Sighting::Unreadable,
            }
        };
        let mut step = machine.sight(sighting);
        while let Step::Return(intruder) = step {
            let restored = give_back(adapter, lease, &machine, &intruder);
            step = machine.returned(restored);
        }
        if step == Step::Done {
            return machine.state();
        }
        std::thread::sleep(POLL);
    }
}

fn give_back(
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
    read(adapter).is_ok_and(|now| {
        now.focused.as_ref().map(|window| &window.id) == Some(&focus.id)
            && prior
                .top
                .as_ref()
                .is_none_or(|top| now.top.as_ref().map(|window| &window.id) == Some(&top.id))
    })
}

#[cfg(test)]
#[path = "front_guard_tests.rs"]
mod tests;
