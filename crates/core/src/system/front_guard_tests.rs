use std::sync::Mutex;
use std::time::Duration;

use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{FrontWindow, InteractionPolicy};

fn win(id: &str, pid: u32) -> FrontWindow {
    FrontWindow { id: id.into(), pid }
}

fn front(id: &str, windows: &[&str]) -> FrontView {
    FrontView {
        focused: Some(win(id, 1)),
        top: Some(win(id, 1)),
        windows: windows.iter().map(|w| (*w).into()).collect(),
    }
}

struct Desk {
    now: Mutex<FrontView>,
    calls: Mutex<Vec<String>>,
    reads: Mutex<u32>,
    obeys: bool,
}

impl Desk {
    fn new(obeys: bool) -> Self {
        Self {
            now: Mutex::new(front("user", &["user"])),
            calls: Mutex::new(Vec::new()),
            reads: Mutex::new(0),
            obeys,
        }
    }

    fn intrude(&self) {
        *self.now.lock().unwrap() = front("new", &["user", "new"]);
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl ObservationOps for Desk {}
impl ActionOps for Desk {}
impl InputOps for Desk {}

impl SystemOps for Desk {
    crate::adapter::guarded_interaction_lease!();

    fn read_front(&self, _deadline: Deadline) -> Result<FrontView, AdapterError> {
        *self.reads.lock().unwrap() += 1;
        Ok(self.now.lock().unwrap().clone())
    }

    fn keep_below(
        &self,
        window: &FrontWindow,
        above: &FrontWindow,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("keep_below {} {}", window.id, above.id));
        Ok(())
    }

    fn give_back(
        &self,
        window: &FrontWindow,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("give_back {}", window.id));
        if self.obeys {
            *self.now.lock().unwrap() = front(&window.id, &["user", "new"]);
        }
        Ok(())
    }
}

fn watch() -> FrontWatch {
    FrontWatch {
        owned_pid: None,
        settle: Duration::from_millis(60),
    }
}

fn lease(desk: &Desk) -> InteractionLease {
    desk.acquire_interaction_lease(Deadline::standard().unwrap())
        .unwrap()
}

#[test]
fn a_window_that_takes_the_focus_during_the_command_is_returned() {
    let desk = Desk::new(true);
    let (value, outcome) = guarded(
        &desk,
        InteractionPolicy::headless(),
        &lease(&desk),
        watch(),
        || {
            desk.intrude();
            std::thread::sleep(Duration::from_millis(100));
            7
        },
    );
    assert_eq!(value, 7);
    assert_eq!(outcome, FrontOutcome::GivenBack);
    assert_eq!(desk.calls(), ["keep_below new user", "give_back user"]);
}

#[test]
fn a_window_that_takes_the_focus_after_the_command_returns_is_returned_inside_the_settle() {
    let desk = Desk::new(true);
    let (_, outcome) = guarded(
        &desk,
        InteractionPolicy::headless(),
        &lease(&desk),
        watch(),
        || {
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(20));
                    desk.intrude();
                });
            });
        },
    );
    assert_eq!(outcome, FrontOutcome::GivenBack);
}

#[test]
fn a_command_that_changes_nothing_ends_clean_with_no_give_back() {
    let desk = Desk::new(true);
    let (_, outcome) = guarded(
        &desk,
        InteractionPolicy::headless(),
        &lease(&desk),
        watch(),
        || (),
    );
    assert_eq!(outcome, FrontOutcome::Clean);
    assert!(desk.calls().is_empty());
}

#[test]
fn a_front_that_cannot_be_returned_reports_the_failure() {
    let desk = Desk::new(false);
    let (_, outcome) = guarded(
        &desk,
        InteractionPolicy::headless(),
        &lease(&desk),
        watch(),
        || {
            desk.intrude();
            std::thread::sleep(Duration::from_millis(150));
        },
    );
    assert_eq!(outcome, FrontOutcome::GiveBackFailed);
    assert!(!outcome.focus_returned());
}

#[test]
fn a_policy_that_allows_focus_steal_runs_the_command_with_no_reads() {
    let desk = Desk::new(true);
    let (_, outcome) = guarded(
        &desk,
        InteractionPolicy::headed(),
        &lease(&desk),
        watch(),
        || {
            desk.intrude();
        },
    );
    assert_eq!(outcome, FrontOutcome::Off);
    assert_eq!(*desk.reads.lock().unwrap(), 0);
    assert!(desk.calls().is_empty());
}

#[test]
fn an_adapter_that_cannot_read_the_front_runs_the_command_unguarded() {
    struct Blind;
    impl ObservationOps for Blind {}
    impl ActionOps for Blind {}
    impl InputOps for Blind {}
    impl SystemOps for Blind {
        crate::adapter::guarded_interaction_lease!();
    }
    let blind = Blind;
    let lease = blind
        .acquire_interaction_lease(Deadline::standard().unwrap())
        .unwrap();
    let (value, outcome) = guarded(&blind, InteractionPolicy::headless(), &lease, watch(), || 3);
    assert_eq!((value, outcome), (3, FrontOutcome::Off));
}

#[test]
fn an_outcome_is_noted_under_focus_returned_and_a_clean_one_adds_nothing() {
    let mut details = None;
    FrontOutcome::Clean.note_in(&mut details);
    assert!(details.is_none());
    FrontOutcome::GivenBack.note_in(&mut details);
    assert_eq!(details.as_ref().unwrap()["focus_returned"], true);
    let mut failed = None;
    FrontOutcome::GiveBackFailed.note_in(&mut failed);
    assert_eq!(failed.as_ref().unwrap()["focus_returned"], false);
    assert!(
        failed.unwrap()["suggestion"]
            .as_str()
            .unwrap()
            .contains("focus-window")
    );
}
