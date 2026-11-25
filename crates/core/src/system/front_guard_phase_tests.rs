use std::time::{Duration, Instant};

use super::tests::{Desk, lease, run};
use super::*;
use crate::InteractionPolicy;

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

#[test]
fn a_window_a_running_instance_opens_for_a_launch_is_returned() {
    let desk = Desk::new(true);
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), |_| {
        desk.intrude_through_a_running_instance();
        std::thread::sleep(Duration::from_millis(100));
    });
    assert_eq!(outcome, FrontOutcome::GivenBack);
    assert_eq!(
        desk.calls(),
        ["keep_below forwarded user", "give_back user"]
    );
}

#[test]
fn the_time_from_an_intrusion_to_its_give_back_is_a_few_polls() {
    let desk = Desk::new(true);
    let mut intruded = None;
    let (_, outcome) = run(&desk, InteractionPolicy::headless(), |_| {
        std::thread::sleep(Duration::from_millis(30));
        intruded = Some(Instant::now());
        desk.intrude();
        std::thread::sleep(Duration::from_millis(150));
    });
    assert_eq!(outcome, FrontOutcome::GivenBack);
    let latency = desk
        .gave_back_at()
        .unwrap()
        .duration_since(intruded.unwrap());
    assert!(
        latency < Duration::from_millis(40),
        "intrusion to give-back: {latency:?}"
    );
}
