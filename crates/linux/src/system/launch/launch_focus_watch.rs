//! Runs the focus watch of a background launch beside its readiness wait. The
//! two share one deadline, so run one after the other the watch would spend
//! the time the readiness wait needs to see the window.

use std::sync::atomic::{AtomicBool, Ordering};

use agent_desktop_core::{Deadline, InteractionLease};

use crate::system::focus::focus_return::{self, Prior};

/// The focus watch of one launch: gives the focus back when the launched
/// application took it, then releases the lease it was handed.
pub(super) fn returning(
    prior: Option<Prior>,
    belongs: impl Fn(u32) -> bool + Send,
    lease: Option<InteractionLease>,
    deadline: Deadline,
) -> impl FnOnce(&AtomicBool) -> bool + Send {
    move |hopeless| {
        let watching = || !hopeless.load(Ordering::Relaxed);
        let returned =
            prior.is_some_and(|prior| focus_return::give_back(&prior, belongs, watching, deadline));
        drop(lease);
        returned
    }
}

/// Runs `watch` on its own thread while `ready` runs on this one and returns
/// both answers. Either side reads or sets the flag that says the launch
/// cannot show a window, which ends the watch early.
pub(super) fn beside<T>(
    watch: impl FnOnce(&AtomicBool) -> bool + Send,
    ready: impl FnOnce(&AtomicBool) -> T,
) -> (bool, T) {
    let hopeless = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let watching = scope.spawn(|| watch(&hopeless));
        let answer = ready(&hopeless);
        (watching.join().unwrap_or(false), answer)
    })
}

#[cfg(test)]
#[path = "launch_focus_watch_tests.rs"]
mod tests;
