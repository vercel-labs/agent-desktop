use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::time::Duration;

use super::beside;

const LONG: Duration = Duration::from_secs(5);

#[test]
fn the_readiness_wait_runs_while_the_focus_watch_is_still_watching() {
    let (ready_ran, watch_hears) = channel();
    let (returned, sent) = beside(
        move |_| watch_hears.recv_timeout(LONG).is_ok(),
        |_| ready_ran.send(()).is_ok(),
    );
    assert!(sent);
    assert!(returned, "the watch ended before the readiness wait ran");
}

#[test]
fn the_watch_ends_when_the_readiness_wait_finds_the_launch_hopeless() {
    let (returned, ()) = beside(
        |hopeless| {
            let give_up = std::time::Instant::now() + LONG;
            while !hopeless.load(Ordering::Relaxed) && std::time::Instant::now() < give_up {
                std::thread::sleep(Duration::from_millis(1));
            }
            hopeless.load(Ordering::Relaxed)
        },
        |hopeless| hopeless.store(true, Ordering::Relaxed),
    );
    assert!(returned);
}
