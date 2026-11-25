//! The decisions of the front guard, with no clock and no platform. A headless
//! command must leave the user's front window and focus as it found them. The
//! machine takes what the adapter reads and says what to do next.
//!
//! An intruder is a window that did not exist before the command, or a window
//! of the process the command targets, that now has the focus or the top. A
//! window that existed before and belongs to someone else is the user's own
//! switch, and the machine never touches it. Window ids decide this, not
//! process ids: an application can fork and detach, and its window still did
//! not exist before.

use super::front_guard_state::FrontGuardState;
use super::front_view::FrontView;
use super::front_window::FrontWindow;

pub(crate) const MAX_GIVE_BACKS: u8 = 3;

pub(crate) enum Sighting {
    Front(FrontView),
    Unreadable,
    Settled,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Poll,
    Return(FrontWindow),
    Done,
}

enum Verdict {
    Quiet,
    Yielded,
    Intruder(FrontWindow),
}

pub(crate) struct FrontGuardMachine {
    prior: FrontView,
    owned_pid: Option<u32>,
    state: FrontGuardState,
    give_backs: u8,
}

impl FrontGuardMachine {
    pub(crate) fn new(prior: FrontView, owned_pid: Option<u32>) -> Self {
        Self {
            prior,
            owned_pid,
            state: FrontGuardState::Watching,
            give_backs: 0,
        }
    }

    pub(crate) fn state(&self) -> FrontGuardState {
        self.state
    }

    pub(crate) fn prior(&self) -> &FrontView {
        &self.prior
    }

    pub(crate) fn sight(&mut self, sighting: Sighting) -> Step {
        match (self.state, sighting) {
            (FrontGuardState::Watching | FrontGuardState::GivenBack, Sighting::Settled) => {
                self.finish();
                Step::Done
            }
            (FrontGuardState::Watching | FrontGuardState::GivenBack, Sighting::Front(now)) => {
                self.judge(&now)
            }
            (FrontGuardState::Watching | FrontGuardState::GivenBack, Sighting::Unreadable) => {
                Step::Poll
            }
            (FrontGuardState::Intruded, _) => Step::Poll,
            (FrontGuardState::Clean | FrontGuardState::GiveBackFailed, _) => Step::Done,
        }
    }

    /// The result of one give-back: whether the read after it showed the
    /// user's front again.
    pub(crate) fn returned(&mut self, restored: bool) -> Step {
        if self.state != FrontGuardState::Intruded {
            return Step::Done;
        }
        self.give_backs += 1;
        if restored {
            self.state = FrontGuardState::GivenBack;
            return Step::Poll;
        }
        if self.give_backs >= MAX_GIVE_BACKS {
            self.state = FrontGuardState::GiveBackFailed;
            return Step::Done;
        }
        self.state = FrontGuardState::Watching;
        Step::Poll
    }

    fn finish(&mut self) {
        if self.state == FrontGuardState::Watching {
            self.state = FrontGuardState::Clean;
        }
    }

    fn judge(&mut self, now: &FrontView) -> Step {
        match self.classify(now) {
            Verdict::Quiet => Step::Poll,
            Verdict::Yielded => {
                self.finish();
                Step::Done
            }
            Verdict::Intruder(_) if self.give_backs >= MAX_GIVE_BACKS => {
                self.state = FrontGuardState::GiveBackFailed;
                Step::Done
            }
            Verdict::Intruder(intruder) => {
                self.state = FrontGuardState::Intruded;
                Step::Return(intruder)
            }
        }
    }

    fn classify(&self, now: &FrontView) -> Verdict {
        let Some(prior_focus) = self.prior.focused.as_ref() else {
            return Verdict::Yielded;
        };
        if !now.windows.is_empty() && !now.has_window(&prior_focus.id) {
            return Verdict::Yielded;
        }
        let ours = |window: &FrontWindow| {
            !self.prior.has_window(&window.id) || self.owned_pid == Some(window.pid)
        };
        let displaced = |window: &&FrontWindow| {
            window.id != prior_focus.id
                && self
                    .prior
                    .top
                    .as_ref()
                    .is_none_or(|top| top.id != window.id)
                && ours(window)
        };
        let intruder = [now.focused.as_ref(), now.top.as_ref()]
            .into_iter()
            .flatten()
            .find(displaced);
        let user_moved_focus = now
            .focused
            .as_ref()
            .is_some_and(|focused| focused.id != prior_focus.id && !ours(focused));
        match intruder {
            Some(_) if user_moved_focus => Verdict::Yielded,
            Some(window) => Verdict::Intruder(window.clone()),
            None if user_moved_focus => Verdict::Yielded,
            None => Verdict::Quiet,
        }
    }
}

#[cfg(test)]
#[path = "front_guard_machine_tests.rs"]
mod tests;
