use std::time::Duration;

use super::tests::{Desk, front, lease};
use super::*;
use crate::InteractionPolicy;

fn watch() -> FrontWatch {
    FrontWatch {
        target: FrontTarget::program("fixture"),
        settle: Duration::from_millis(60),
        after_release: true,
    }
}

fn open_a_window_over(desk: &Desk) -> (FrontOutcome, bool) {
    let (_, outcome, pending) = guarded_deferred(
        desk,
        InteractionPolicy::headless(),
        &lease(desk),
        watch(),
        |mark| {
            mark.dispatched();
            *desk.now.lock().unwrap() = front("new", &["user", "new"]);
            std::thread::sleep(Duration::from_millis(100));
        },
    )
    .unwrap();
    (outcome, pending.is_some())
}

#[test]
fn a_window_a_command_opens_when_nothing_was_focused_stays_in_front() {
    let desk = Desk::new(true);
    *desk.now.lock().unwrap() = FrontView {
        focused: None,
        top: None,
        windows: vec!["user".into()],
        ..FrontView::default()
    };
    let (outcome, pending) = open_a_window_over(&desk);
    assert_eq!(outcome, FrontOutcome::Off);
    assert!(!pending);
    assert!(desk.calls().is_empty(), "no keep_below, no give_back");
    let now = desk.now.lock().unwrap();
    assert_eq!(now.focused.as_ref().unwrap().id, "new");
    assert_eq!(now.top.as_ref().unwrap().id, "new");
}

#[test]
fn the_same_window_over_another_applications_focused_window_is_given_back() {
    let desk = Desk::new(true);
    let (outcome, _) = open_a_window_over(&desk);
    assert_eq!(outcome, FrontOutcome::GivenBack);
    assert_eq!(desk.calls(), ["keep_below new user", "give_back user"]);
}
