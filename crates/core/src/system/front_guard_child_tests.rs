use std::sync::Arc;
use std::time::{Duration, Instant};

use super::tests::{Desk, front};
use super::*;
use crate::InteractionPolicy;
use crate::adapter::SystemOps;

const TARGET_PID: u32 = 50;

fn watch(after_release: bool) -> FrontWatch {
    FrontWatch {
        target: FrontTarget {
            pid: Some(TARGET_PID),
            image: None,
            forwarded: false,
        },
        settle: Duration::from_millis(60),
        after_release,
    }
}

fn lease_of(desk: &Desk, ms: u64) -> InteractionLease {
    desk.acquire_interaction_lease(Deadline::detached_after(ms).unwrap())
        .unwrap()
}

fn run(
    desk: &Desk,
    children: bool,
    lease_ms: u64,
) -> (FrontOutcome, Option<FrontPending>, InteractionLease) {
    let lease = lease_of(desk, lease_ms);
    let (_, outcome, pending) = guarded_deferred(
        desk,
        InteractionPolicy::headless(),
        &lease,
        watch(children),
        |mark| {
            mark.dispatched();
            *desk.kids.lock().unwrap() = vec![300];
        },
    )
    .unwrap();
    (outcome, pending, lease)
}

fn map_child_window_after(desk: &Arc<Desk>, delay_ms: u64) -> std::thread::JoinHandle<()> {
    let desk = Arc::clone(desk);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(delay_ms));
        *desk.now.lock().unwrap() = front("child", &["user", "child"]);
    })
}

#[test]
fn a_window_a_spawned_helper_maps_after_the_settle_is_given_back_once_the_lease_is_released() {
    let desk = Arc::new(Desk::new(true));
    let mapper = map_child_window_after(&desk, 400);
    let (outcome, pending, lease) = run(&desk, true, 5_000);
    assert_eq!(outcome, FrontOutcome::Clean);
    let pending = pending.expect("a helper is running with no window yet");
    drop(lease);
    let started = Instant::now();
    let resolution = pending.finish(&*desk);
    mapper.join().unwrap();
    assert_eq!(resolution.outcome, FrontOutcome::GivenBack);
    assert_eq!(resolution.pending_reason, None);
    assert!(started.elapsed() > Duration::from_millis(250));
    assert_eq!(desk.calls(), ["keep_below child user", "give_back user"]);
}

#[test]
fn a_watch_that_ignores_spawned_helpers_ends_with_the_window_still_in_front() {
    let desk = Arc::new(Desk::new(true));
    let mapper = map_child_window_after(&desk, 400);
    let (outcome, pending, _lease) = run(&desk, false, 5_000);
    assert_eq!((outcome, pending.is_none()), (FrontOutcome::Clean, true));
    mapper.join().unwrap();
    assert_eq!(
        desk.now.lock().unwrap().focused.as_ref().unwrap().id,
        "child"
    );
    assert!(desk.calls().is_empty());
}

#[test]
fn an_action_that_spawns_nothing_ends_at_dispatch_plus_the_settle() {
    let desk = Desk::new(true);
    let lease = lease_of(&desk, 5_000);
    let started = Instant::now();
    let (_, outcome, pending) = guarded_deferred(
        &desk,
        InteractionPolicy::headless(),
        &lease,
        watch(true),
        |mark| mark.dispatched(),
    )
    .unwrap();
    assert_eq!(outcome, FrontOutcome::Clean);
    let resolution = pending.unwrap().finish(&desk);
    assert_eq!(resolution.outcome, FrontOutcome::Clean);
    assert_eq!(resolution.pending_reason, None);
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "empty watch took {elapsed:?} instead of ending at dispatch plus the 60 ms settle"
    );
}

#[test]
fn a_quiet_window_of_a_spawned_helper_ends_the_watch_at_seen_plus_the_settle() {
    let desk = Arc::new(Desk::new(true));
    let (_, pending, lease) = run(&desk, true, 5_000);
    drop(lease);
    let quiet = Arc::clone(&desk);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        let mut view = front("user", &["user", "child"]);
        view.owners = vec![1, 300];
        *quiet.now.lock().unwrap() = view;
    });
    let started = Instant::now();
    let resolution = pending.unwrap().finish(&*desk);
    assert_eq!(resolution.outcome, FrontOutcome::Clean);
    assert_eq!(resolution.pending_reason, None);
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_millis(350) && waited < Duration::from_millis(900),
        "seen at about 150 ms plus the 250 ms settle, not the cap: {waited:?}"
    );
    assert!(desk.calls().is_empty());
}

#[test]
fn a_helper_that_exits_without_a_window_ends_the_watch() {
    let desk = Arc::new(Desk::new(true));
    let (outcome, pending, lease) = run(&desk, true, 5_000);
    assert_eq!(outcome, FrontOutcome::Clean);
    drop(lease);
    let ends = Arc::clone(&desk);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        ends.kids.lock().unwrap().clear();
    });
    let started = Instant::now();
    let resolution = pending.unwrap().finish(&*desk);
    assert_eq!(resolution.outcome, FrontOutcome::Clean);
    assert_eq!(resolution.pending_reason, None);
    assert!(started.elapsed() < Duration::from_millis(600));
}

#[test]
fn a_helper_still_running_at_the_command_deadline_is_reported_as_pending() {
    let desk = Arc::new(Desk::new(true));
    let (_, pending, lease) = run(&desk, true, 700);
    drop(lease);
    let started = Instant::now();
    let resolution = pending.unwrap().finish(&*desk);
    assert_eq!(resolution.outcome, FrontOutcome::Clean);
    assert_eq!(resolution.pending_reason, Some("child_window_pending"));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn an_intrusion_while_the_post_release_reads_run_is_given_back_when_it_is_seen() {
    let desk = Arc::new(Desk::new(true));
    let lease = lease_of(&desk, 5_000);
    let (_, outcome, pending) = guarded_deferred(
        &*desk,
        InteractionPolicy::headless(),
        &lease,
        watch(true),
        |mark| mark.dispatched(),
    )
    .unwrap();
    assert_eq!(outcome, FrontOutcome::Clean);
    drop(lease);
    *desk.kids.lock().unwrap() = vec![300];
    let mut reads_done = None;
    let ((), resolution) = pending.unwrap().run_around(&*desk, || {
        std::thread::sleep(Duration::from_millis(120));
        *desk.now.lock().unwrap() = front("child", &["user", "child"]);
        std::thread::sleep(Duration::from_millis(500));
        reads_done = Some(Instant::now());
    });
    assert_eq!(resolution.outcome, FrontOutcome::GivenBack);
    let gave_back = desk.gave_back_at().expect("a give-back");
    let reads_done = reads_done.unwrap();
    assert!(
        gave_back < reads_done,
        "give-back happened {:?} after post-release reads finished instead of while they ran",
        gave_back.saturating_duration_since(reads_done)
    );
}

#[test]
fn the_watch_around_post_release_work_adds_no_time_when_the_work_outlasts_the_settle() {
    let desk = Desk::new(true);
    let lease = lease_of(&desk, 5_000);
    let mut cfg = watch(true);
    cfg.settle = Duration::from_millis(450);
    let (_, _, pending) =
        guarded_deferred(&desk, InteractionPolicy::headless(), &lease, cfg, |mark| {
            mark.dispatched()
        })
        .unwrap();
    drop(lease);
    let mut work_done = None;
    let (_, resolution) = pending.unwrap().run_around(&desk, || {
        std::thread::sleep(Duration::from_millis(550));
        work_done = Some(Instant::now());
    });
    assert_eq!(resolution.outcome, FrontOutcome::Clean);
    let tail = work_done.unwrap().elapsed();
    assert!(
        tail < Duration::from_millis(300),
        "watch waited {tail:?} after post-release work that already outlasted the 450 ms settle"
    );
}
