use super::*;
use crate::system::x11::X11Client;

fn client(id: u32, pid: u32) -> X11Client {
    X11Client {
        id,
        pid: Some(pid),
        title: String::new(),
        extents: None,
        bounds: None,
        hidden: false,
        shadow: (0, 0, 0, 0),
    }
}

fn prior() -> Prior {
    Prior {
        client: 1,
        owner: ProcessIdentity::new(10, "i"),
        known: HashSet::from([1]),
    }
}

fn state(active: u32, clients: Vec<X11Client>) -> X11State {
    X11State::Ready {
        active: Some(active),
        clients,
    }
}

fn launched(pid: u32) -> bool {
    pid == 20
}

#[test]
fn a_new_window_of_the_launch_that_is_active_took_the_focus() {
    let now = state(2, vec![client(1, 10), client(2, 20)]);
    assert_eq!(judge(&prior(), &now, &launched), Look::Stolen);
}

#[test]
fn a_window_the_user_switched_to_is_never_taken_back() {
    let now = state(3, vec![client(1, 10), client(2, 20), client(3, 30)]);
    assert_eq!(judge(&prior(), &now, &launched), Look::Appeared);
}

#[test]
fn a_prior_window_that_closed_has_no_focus_to_return() {
    let now = state(2, vec![client(2, 20)]);
    assert_eq!(judge(&prior(), &now, &launched), Look::Appeared);
}

#[test]
fn an_old_window_of_the_application_is_not_the_launch() {
    let mut old = prior();
    old.known.insert(2);
    let now = state(2, vec![client(1, 10), client(2, 20)]);
    assert_eq!(judge(&old, &now, &launched), Look::Nothing);
}

struct Fake {
    active: std::cell::Cell<u32>,
    sends: std::cell::Cell<usize>,
    reactivated: usize,
}

impl Fake {
    fn stolen(reactivated: usize) -> Self {
        Self {
            active: std::cell::Cell::new(2),
            sends: std::cell::Cell::new(0),
            reactivated,
        }
    }
}

impl Desktop for Fake {
    fn state(&self, _: Deadline) -> X11State {
        state(self.active.get(), vec![client(1, 10), client(2, 20)])
    }

    fn activate(&self, _: &Prior, _: Deadline) -> bool {
        self.sends.set(self.sends.get() + 1);
        if self.sends.get() > self.reactivated {
            self.active.set(1);
        }
        true
    }

    fn active(&self) -> Option<u32> {
        Some(self.active.get())
    }
}

fn deadline() -> Deadline {
    Deadline::after(10_000).unwrap()
}

#[test]
fn an_app_that_reactivates_itself_is_asked_again_until_the_focus_holds() {
    let desktop = Fake::stolen(1);
    let returned = give_back_in(&desktop, &prior(), &launched, || true, deadline());
    assert!(returned);
    assert_eq!(desktop.sends.get(), 2);
}

#[test]
fn an_app_that_keeps_winning_is_asked_a_bounded_number_of_times() {
    let desktop = Fake::stolen(usize::MAX);
    let returned = give_back_in(&desktop, &prior(), &launched, || true, deadline());
    assert!(!returned);
    assert_eq!(desktop.sends.get(), RESTORE_ATTEMPTS);
}

#[test]
fn a_launch_that_cannot_show_a_window_ends_the_wait_at_once() {
    let desktop = Fake::stolen(0);
    desktop.active.set(1);
    let begun = Instant::now();
    let returned = give_back_in(&desktop, &prior(), &launched, || false, deadline());
    assert!(!returned);
    assert!(begun.elapsed() < Duration::from_millis(500));
    assert_eq!(desktop.sends.get(), 0);
}
