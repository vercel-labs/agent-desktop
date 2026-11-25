//! Runs a headless command with a front guard beside it. The guard reads the
//! user's front before the command, watches on its own thread while the
//! command runs and for a short settle after it, and gives the focus and the
//! window order back when a window that is not the user's took them. The
//! first read happens before the command starts, so no intrusion can go
//! unseen for the command's whole run.
//!
//! The adapter supplies three primitives: `read_front`, `keep_below` and
//! `give_back`. Everything that decides is in [`FrontGuardMachine`].

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::front_guard_machine::{FrontGuardMachine, Owner, Sighting, Step};
use super::front_guard_state::FrontGuardState;
use super::front_outcome::FrontOutcome;
use super::front_process::FrontProcess;
use super::front_target::FrontTarget;
use super::front_view::FrontView;
use crate::{
    AdapterError, Deadline, ErrorCode, InteractionLease, InteractionPolicy,
    adapter::PlatformAdapter,
};

const POLL: Duration = Duration::from_millis(20);
const READ_BUDGET_MS: u64 = 500;

/// What one guarded command asks of the guard.
#[derive(Clone)]
pub(crate) struct FrontWatch {
    /// The process the command acts on, and so whose windows are intruders.
    pub target: FrontTarget,
    /// How long the guard keeps watching after the command returns.
    pub settle: Duration,
}

/// Runs `command` under the guard. A policy that lets a window take the focus
/// runs it with no guard and no reads, and so does an adapter that has no way
/// to read the front. An adapter that has one and fails to read it refuses the
/// command before it starts: a guard that cannot see the front cannot keep it.
pub(crate) fn guarded<T>(
    adapter: &dyn PlatformAdapter,
    policy: InteractionPolicy,
    lease: &InteractionLease,
    watch: FrontWatch,
    command: impl FnOnce() -> T,
) -> Result<(T, FrontOutcome), AdapterError> {
    if policy.allow_focus_steal {
        return Ok((command(), FrontOutcome::Off));
    }
    let prior = match read(adapter) {
        Ok(view) if view.focused.is_some() => view,
        Ok(_) => return Ok((command(), FrontOutcome::Off)),
        Err(error) if error.code == ErrorCode::PlatformNotSupported => {
            return Ok((command(), FrontOutcome::Off));
        }
        Err(_) => return Err(unreadable(policy)),
    };
    let captured_ms = unix_ms();
    let processes = Mutex::new(HashMap::<u32, Option<FrontProcess>>::new());
    let processes = &processes;
    let target = watch.target.clone();
    let owner: Owner<'_> = Box::new(move |window, is_new| {
        target.owns(
            window,
            is_new,
            || {
                processes.lock().map_or(None, |mut known| {
                    known
                        .entry(window.pid)
                        .or_insert_with(|| adapter.front_process(window.pid))
                        .clone()
                })
            },
            captured_ms,
        )
    });
    let machine = FrontGuardMachine::new(prior, owner);
    let finished = AtomicBool::new(false);
    let watching = lease.with_deadline(lease.deadline());
    Ok(std::thread::scope(|scope| {
        let guard = scope.spawn(|| keep_watching(adapter, &watching, machine, &watch, &finished));
        let value = command();
        finished.store(true, Ordering::Release);
        let outcome = guard
            .join()
            .map_or(FrontOutcome::Off, FrontOutcome::from_state);
        (value, outcome)
    }))
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

fn read(adapter: &dyn PlatformAdapter) -> Result<FrontView, AdapterError> {
    adapter.read_front(Deadline::detached_after(READ_BUDGET_MS)?)
}

fn keep_watching(
    adapter: &dyn PlatformAdapter,
    lease: &InteractionLease,
    mut machine: FrontGuardMachine<'_>,
    watch: &FrontWatch,
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
    machine: &FrontGuardMachine<'_>,
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
