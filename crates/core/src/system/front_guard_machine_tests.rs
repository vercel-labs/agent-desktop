use super::*;

fn win(id: &str, pid: u32) -> FrontWindow {
    FrontWindow { id: id.into(), pid }
}

fn view(focused: &str, top: &str, windows: &[&str]) -> FrontView {
    let pid_of = |id: &str| if id.starts_with('n') { 900 } else { 100 };
    FrontView {
        focused: Some(win(focused, pid_of(focused))),
        top: Some(win(top, pid_of(top))),
        windows: windows.iter().map(|id| (*id).into()).collect(),
    }
}

fn prior() -> FrontView {
    view("user", "user", &["user", "other"])
}

fn machine() -> FrontGuardMachine {
    FrontGuardMachine::new(prior(), None)
}

#[test]
fn an_unchanged_front_polls_and_settles_clean() {
    let mut guard = machine();
    assert_eq!(guard.sight(Sighting::Front(prior())), Step::Poll);
    assert_eq!(guard.sight(Sighting::Unreadable), Step::Poll);
    assert_eq!(guard.state(), FrontGuardState::Watching);
    assert_eq!(guard.sight(Sighting::Settled), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::Clean);
}

#[test]
fn a_new_window_that_takes_the_focus_is_returned_whoever_owns_it() {
    let mut guard = machine();
    let now = view("n1", "n1", &["user", "other", "n1"]);
    assert_eq!(
        guard.sight(Sighting::Front(now)),
        Step::Return(win("n1", 900))
    );
    assert_eq!(guard.state(), FrontGuardState::Intruded);
    assert_eq!(guard.returned(true), Step::Poll);
    assert_eq!(guard.state(), FrontGuardState::GivenBack);
    assert_eq!(guard.sight(Sighting::Settled), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::GivenBack);
}

#[test]
fn a_new_window_on_top_with_the_focus_unchanged_is_returned() {
    let mut guard = machine();
    let now = view("user", "n1", &["user", "other", "n1"]);
    assert_eq!(
        guard.sight(Sighting::Front(now)),
        Step::Return(win("n1", 900))
    );
}

#[test]
fn a_switch_to_a_window_that_existed_before_is_left_alone() {
    let mut guard = machine();
    let now = view("other", "other", &["user", "other"]);
    assert_eq!(guard.sight(Sighting::Front(now)), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::Clean);
}

#[test]
fn a_switch_to_an_old_window_beats_a_new_window_on_top() {
    let mut guard = machine();
    let now = view("other", "n1", &["user", "other", "n1"]);
    assert_eq!(guard.sight(Sighting::Front(now)), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::Clean);
}

#[test]
fn an_old_window_of_the_targeted_process_that_takes_the_focus_is_returned() {
    let mut guard = FrontGuardMachine::new(prior(), Some(100));
    let now = view("other", "other", &["user", "other"]);
    assert_eq!(
        guard.sight(Sighting::Front(now)),
        Step::Return(win("other", 100))
    );
}

#[test]
fn a_prior_window_that_closed_is_not_returned_to() {
    let mut guard = machine();
    let now = view("other", "other", &["other"]);
    assert_eq!(guard.sight(Sighting::Front(now)), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::Clean);
}

#[test]
fn a_failed_give_back_tries_again_then_gives_up() {
    let mut guard = machine();
    let now = || view("n1", "n1", &["user", "other", "n1"]);
    for _ in 0..2 {
        assert!(matches!(
            guard.sight(Sighting::Front(now())),
            Step::Return(_)
        ));
        assert_eq!(guard.returned(false), Step::Poll);
        assert_eq!(guard.state(), FrontGuardState::Watching);
    }
    assert!(matches!(
        guard.sight(Sighting::Front(now())),
        Step::Return(_)
    ));
    assert_eq!(guard.returned(false), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::GiveBackFailed);
    assert_eq!(guard.sight(Sighting::Front(now())), Step::Done);
}

#[test]
fn an_application_that_takes_the_focus_again_after_a_give_back_is_returned_again() {
    let mut guard = machine();
    let now = || view("n1", "n1", &["user", "other", "n1"]);
    assert!(matches!(
        guard.sight(Sighting::Front(now())),
        Step::Return(_)
    ));
    guard.returned(true);
    assert!(matches!(
        guard.sight(Sighting::Front(now())),
        Step::Return(_)
    ));
    assert_eq!(guard.state(), FrontGuardState::Intruded);
}

#[test]
fn the_budget_of_give_backs_is_shared_across_intrusions() {
    let mut guard = machine();
    let now = || view("n1", "n1", &["user", "other", "n1"]);
    for _ in 0..3 {
        assert!(matches!(
            guard.sight(Sighting::Front(now())),
            Step::Return(_)
        ));
        guard.returned(true);
    }
    assert_eq!(guard.sight(Sighting::Front(now())), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::GiveBackFailed);
}

#[test]
fn a_guard_with_nobody_focused_before_the_command_has_nothing_to_return_to() {
    let mut guard = FrontGuardMachine::new(FrontView::default(), None);
    let now = view("n1", "n1", &["n1"]);
    assert_eq!(guard.sight(Sighting::Front(now)), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::Clean);
}
