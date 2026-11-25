use std::sync::Mutex;
use std::time::Duration;

use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{FrontWindow, InteractionPolicy};

fn win(id: &str, pid: u32) -> FrontWindow {
    FrontWindow { id: id.into(), pid }
}

fn pid_of(id: &str) -> u32 {
    match id {
        "new" => 9,
        "mine" => 5,
        _ => 1,
    }
}

fn front(id: &str, windows: &[&str]) -> FrontView {
    FrontView {
        focused: Some(win(id, pid_of(id))),
        top: Some(win(id, pid_of(id))),
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

    fn user_opens_a_window(&self) {
        *self.now.lock().unwrap() = front("mine", &["user", "mine"]);
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

    fn front_process(&self, pid: u32) -> Option<crate::FrontProcess> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis() as u64;
        match pid {
            9 => Some(crate::FrontProcess {
                image: "fixture".into(),
                started_ms: now,
            }),
            5 => Some(crate::FrontProcess {
                image: "editor".into(),
                started_ms: now,
            }),
            _ => None,
        }
    }

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
        target: FrontTarget::program("fixture"),
        settle: Duration::from_millis(60),
    }
}

fn lease(desk: &impl SystemOps) -> InteractionLease {
    desk.acquire_interaction_lease(Deadline::standard().unwrap())
        .unwrap()
}

fn run<T>(
    desk: &Desk,
    policy: InteractionPolicy,
    command: impl FnOnce() -> T,
) -> (T, FrontOutcome) {
    guarded(desk, policy, &lease(desk), watch(), command).expect("the front is readable")
}

#[test]
fn a_window_that_takes_the_focus_during_the_command_is_returned() {
    let desk = Desk::new(true);
    let (value, outcome) = run(&desk, InteractionPolicy::headless(), || {
        desk.intrude();
        std::thread::sleep(Duration::from_millis(100));
        7
    });
    assert_eq!(value, 7);
    assert_eq!(outcome, FrontOutcome::GivenBack);
    assert_eq!(desk.calls(), ["keep_below new user", "give_back user"]);
}

#[test]
fn a_window_that_takes_the_focus_after_the_command_returns_is_returned_inside_the_settle() {
    let desk = Desk::new(true);
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), || {
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(20));
                desk.intrude();
            });
        });
    });
    assert_eq!(outcome, FrontOutcome::GivenBack);
}

#[test]
fn a_window_the_user_opens_during_the_command_is_never_given_back() {
    let desk = Desk::new(true);
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), || {
        desk.user_opens_a_window();
        std::thread::sleep(Duration::from_millis(100));
    });
    assert_eq!(outcome, FrontOutcome::Clean);
    assert!(desk.calls().is_empty());
}

#[test]
fn a_command_that_changes_nothing_ends_clean_with_no_give_back() {
    let desk = Desk::new(true);
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), || ());
    assert_eq!(outcome, FrontOutcome::Clean);
    assert!(desk.calls().is_empty());
}

#[test]
fn a_front_that_cannot_be_returned_reports_the_failure() {
    let desk = Desk::new(false);
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), || {
        desk.intrude();
        std::thread::sleep(Duration::from_millis(150));
    });
    assert_eq!(outcome, FrontOutcome::GiveBackFailed);
    assert!(!outcome.focus_returned());
}

#[test]
fn a_policy_that_allows_focus_steal_runs_the_command_with_no_reads() {
    let desk = Desk::new(true);
    let (_, outcome) = run(&desk, InteractionPolicy::headed(), || desk.intrude());
    assert_eq!(outcome, FrontOutcome::Off);
    assert_eq!(*desk.reads.lock().unwrap(), 0);
    assert!(desk.calls().is_empty());
}

struct Blind(AdapterError);
impl ObservationOps for Blind {}
impl ActionOps for Blind {}
impl InputOps for Blind {}
impl SystemOps for Blind {
    crate::adapter::guarded_interaction_lease!();

    fn read_front(&self, _deadline: Deadline) -> Result<FrontView, AdapterError> {
        Err(self.0.clone())
    }
}

#[test]
fn an_adapter_with_no_way_to_read_the_front_runs_the_command_unguarded() {
    let blind = Blind(AdapterError::not_supported("read_front"));
    let (value, outcome) = guarded(
        &blind,
        InteractionPolicy::headless(),
        &lease(&blind),
        watch(),
        || 3,
    )
    .unwrap();
    assert_eq!((value, outcome), (3, FrontOutcome::Off));
}

#[test]
fn an_adapter_that_fails_to_read_the_front_refuses_the_headless_command_unsent() {
    let blind = Blind(AdapterError::app_unresponsive("display"));
    let ran = std::sync::atomic::AtomicBool::new(false);
    let error = guarded(
        &blind,
        InteractionPolicy::headless(),
        &lease(&blind),
        watch(),
        || ran.store(true, Ordering::SeqCst),
    )
    .expect_err("a refusal");
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(error.details.as_ref().unwrap()["kind"], "focus_unreadable");
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert!(!ran.load(Ordering::SeqCst));
}

#[test]
fn a_failed_read_does_not_refuse_a_command_the_policy_lets_take_the_focus() {
    let blind = Blind(AdapterError::app_unresponsive("display"));
    let (value, outcome) = guarded(
        &blind,
        InteractionPolicy::headed(),
        &lease(&blind),
        watch(),
        || 3,
    )
    .unwrap();
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
