use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

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
    gave_back_at: Mutex<Option<Instant>>,
    obeys: bool,
}

impl Desk {
    fn new(obeys: bool) -> Self {
        Self {
            now: Mutex::new(front("user", &["user"])),
            calls: Mutex::new(Vec::new()),
            reads: Mutex::new(0),
            gave_back_at: Mutex::new(None),
            obeys,
        }
    }

    fn user_opens_a_window(&self) {
        *self.now.lock().unwrap() = front("mine", &["user", "mine"]);
    }

    fn intrude_top_only(&self) {
        let mut view = front("user", &["user", "new"]);
        view.top = Some(win("new", pid_of("new")));
        *self.now.lock().unwrap() = view;
    }

    fn intrude(&self) {
        *self.now.lock().unwrap() = front("new", &["user", "new"]);
    }

    fn gave_back_at(&self) -> Option<Instant> {
        *self.gave_back_at.lock().unwrap()
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
        self.gave_back_at
            .lock()
            .unwrap()
            .get_or_insert_with(Instant::now);
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
    command: impl FnOnce(&DispatchMark) -> T,
) -> (T, FrontOutcome) {
    guarded(desk, policy, &lease(desk), watch(), command).expect("the front is readable")
}

#[test]
fn a_window_that_takes_the_focus_during_the_command_is_returned() {
    let desk = Desk::new(true);
    let (value, outcome) = run(&desk, InteractionPolicy::headless(), |_| {
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
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), |_| {
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
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), |_| {
        desk.user_opens_a_window();
        std::thread::sleep(Duration::from_millis(100));
    });
    assert_eq!(outcome, FrontOutcome::Clean);
    assert!(desk.calls().is_empty());
}

#[test]
fn a_command_that_changes_nothing_ends_clean_with_no_give_back() {
    let desk = Desk::new(true);
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), |_| ());
    assert_eq!(outcome, FrontOutcome::Clean);
    assert!(desk.calls().is_empty());
}

#[test]
fn a_front_that_cannot_be_returned_reports_the_failure() {
    let desk = Desk::new(false);
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), |_| {
        desk.intrude();
        std::thread::sleep(Duration::from_millis(1_000));
    });
    assert_eq!(outcome, FrontOutcome::GiveBackFailed);
    assert!(!outcome.focus_returned());
}

#[test]
fn a_policy_that_allows_focus_steal_runs_the_command_with_no_reads() {
    let desk = Desk::new(true);
    let (_, outcome) = run(&desk, InteractionPolicy::headed(), |_| desk.intrude());
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
        |_| 3,
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
        |_| ran.store(true, Ordering::SeqCst),
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
        |_| 3,
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

fn watch_for(settle_ms: u64) -> FrontWatch {
    FrontWatch {
        target: FrontTarget::program("fixture"),
        settle: Duration::from_millis(settle_ms),
    }
}

#[test]
fn the_settle_runs_from_the_dispatch_beside_the_reads_after_it() {
    let desk = Desk::new(true);
    let started = Instant::now();
    let (_, outcome) = guarded(
        &desk,
        InteractionPolicy::headless(),
        &lease(&desk),
        watch_for(200),
        |mark| {
            mark.dispatched();
            std::thread::sleep(Duration::from_millis(150));
        },
    )
    .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(outcome, FrontOutcome::Clean);
    assert!(
        elapsed >= Duration::from_millis(195) && elapsed < Duration::from_millis(245),
        "the watch ends at dispatch plus the settle, not at return plus the settle: {elapsed:?}"
    );
}

#[test]
fn a_command_that_outlasts_the_settle_ends_the_watch_at_its_return() {
    let desk = Desk::new(true);
    let started = Instant::now();
    guarded(
        &desk,
        InteractionPolicy::headless(),
        &lease(&desk),
        watch_for(50),
        |mark| {
            mark.dispatched();
            std::thread::sleep(Duration::from_millis(150));
        },
    )
    .unwrap();
    assert!(started.elapsed() < Duration::from_millis(190));
}

#[test]
fn a_window_that_opens_after_the_return_but_inside_dispatch_plus_settle_is_returned() {
    let desk = Desk::new(true);
    let (_, outcome) = guarded(
        &desk,
        InteractionPolicy::headless(),
        &lease(&desk),
        watch_for(250),
        |mark| {
            mark.dispatched();
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(150));
                    desk.intrude();
                });
                std::thread::sleep(Duration::from_millis(20));
            });
        },
    )
    .unwrap();
    assert_eq!(outcome, FrontOutcome::GivenBack);
}

#[test]
fn an_intrusion_is_given_back_when_it_is_seen_not_when_the_command_returns() {
    let desk = Desk::new(true);
    let started = Instant::now();
    let mut returned = None;
    let (_, outcome) = guarded(
        &desk,
        InteractionPolicy::headless(),
        &lease(&desk),
        watch_for(60),
        |mark| {
            mark.dispatched();
            std::thread::sleep(Duration::from_millis(40));
            desk.intrude_top_only();
            std::thread::sleep(Duration::from_millis(400));
            returned = Some(Instant::now());
        },
    )
    .unwrap();
    assert_eq!(outcome, FrontOutcome::GivenBack);
    let gave_back = desk
        .gave_back_at()
        .expect("a give-back")
        .duration_since(started);
    let returned = returned.unwrap().duration_since(started);
    assert!(
        gave_back < Duration::from_millis(200) && gave_back + Duration::from_millis(150) < returned,
        "given back at {gave_back:?}, command returned at {returned:?}"
    );
}
