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
        ..Default::default()
    }
}

fn prior() -> FrontView {
    view("user", "user", &["user", "other"])
}

fn machine() -> FrontGuardMachine {
    FrontGuardMachine::new(
        prior(),
        Box::new(|window, is_new| is_new && window.pid == 900),
    )
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
fn a_new_window_of_the_target_that_takes_the_focus_is_returned() {
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
    let mut guard = FrontGuardMachine::new(prior(), Box::new(|window, _| window.pid == 100));
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
    let mut guard = FrontGuardMachine::new(FrontView::default(), Box::new(|_, _| true));
    let now = view("n1", "n1", &["n1"]);
    assert_eq!(guard.sight(Sighting::Front(now)), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::Clean);
}

fn users_window(focused: &str, top: &str) -> FrontView {
    let mut view = view(focused, top, &["user", "other", "mine"]);
    for window in [view.focused.as_mut(), view.top.as_mut()]
        .into_iter()
        .flatten()
    {
        if window.id == "mine" {
            window.pid = 500;
        }
    }
    view
}

#[test]
fn a_new_window_nobody_owns_yet_that_takes_the_focus_keeps_the_watch_going() {
    let mut guard = machine();
    assert_eq!(
        guard.sight(Sighting::Front(users_window("mine", "mine"))),
        Step::Poll
    );
    assert_eq!(guard.state(), FrontGuardState::Watching);
    assert_eq!(guard.sight(Sighting::Settled), Step::Done);
    assert_eq!(guard.state(), FrontGuardState::Clean);
}

#[test]
fn a_new_window_that_becomes_owned_while_it_holds_the_focus_is_returned() {
    let owned = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let known = std::sync::Arc::clone(&owned);
    let mut guard = FrontGuardMachine::new(
        prior(),
        Box::new(move |window, is_new| {
            is_new && window.pid == 500 && known.load(std::sync::atomic::Ordering::SeqCst)
        }),
    );
    assert_eq!(
        guard.sight(Sighting::Front(users_window("mine", "mine"))),
        Step::Poll
    );
    owned.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        guard.sight(Sighting::Front(users_window("mine", "mine"))),
        Step::Return(_)
    ));
}

#[test]
fn a_new_window_the_user_opened_on_top_is_not_an_intruder() {
    let mut guard = machine();
    assert_eq!(
        guard.sight(Sighting::Front(users_window("user", "mine"))),
        Step::Poll
    );
    assert_eq!(guard.state(), FrontGuardState::Watching);
}

#[test]
fn a_user_window_after_a_give_back_is_still_not_given_back() {
    let mut guard = machine();
    let now = view("n1", "n1", &["user", "other", "n1"]);
    assert!(matches!(guard.sight(Sighting::Front(now)), Step::Return(_)));
    guard.returned(true);
    assert_eq!(
        guard.sight(Sighting::Front(users_window("mine", "mine"))),
        Step::Poll
    );
    assert_eq!(guard.state(), FrontGuardState::GivenBack);
}
