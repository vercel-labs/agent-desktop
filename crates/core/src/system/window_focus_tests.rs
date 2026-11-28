use super::*;

#[test]
fn focus_poll_interval_matches_production() {
    assert_eq!(FOCUS_POLL_INTERVAL_MS, 50);
}

use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::time::Instant;

struct LateFocusAdapter {
    started: Instant,
    lands_after: Duration,
    target: WindowInfo,
}

impl ObservationOps for LateFocusAdapter {}
impl ActionOps for LateFocusAdapter {}
impl InputOps for LateFocusAdapter {}

impl SystemOps for LateFocusAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn focused_window(
        &self,
        _deadline: crate::Deadline,
    ) -> Result<Option<WindowInfo>, AdapterError> {
        Ok((self.started.elapsed() >= self.lands_after).then(|| self.target.clone()))
    }
}

fn target_window() -> WindowInfo {
    WindowInfo {
        id: "w1".into(),
        title: "Main".into(),
        app: "Fixture".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: Default::default(),
    }
}

#[test]
fn focus_that_lands_after_the_old_settle_cap_is_still_confirmed() {
    let adapter = LateFocusAdapter {
        started: Instant::now(),
        lands_after: Duration::from_millis(1_100),
        target: target_window(),
    };

    let focused = wait_for_focused_window_with_poll_interval(
        &adapter,
        "w1",
        None,
        Duration::from_millis(50),
        crate::Deadline::standard().unwrap(),
    )
    .unwrap();

    assert_eq!(focused.id, "w1");
}

#[test]
fn focus_that_never_lands_fails_at_the_command_deadline() {
    let adapter = LateFocusAdapter {
        started: Instant::now(),
        lands_after: Duration::from_secs(60),
        target: target_window(),
    };
    let started = Instant::now();

    let error = wait_for_focused_window_with_poll_interval(
        &adapter,
        "w1",
        None,
        Duration::from_millis(20),
        crate::Deadline::after(300).unwrap(),
    )
    .unwrap_err();

    assert!(started.elapsed() >= Duration::from_millis(250));
    let AppError::Adapter(error) = error else {
        panic!("a settle failure is an adapter error");
    };
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn focus_wait_transitions_cover_every_state_and_read() {
    use FocusRead::{OffTarget, OnTarget};
    use FocusWait::{AwaitingFocus, Settled, Stabilizing};
    assert_eq!(AwaitingFocus.advance(OffTarget), AwaitingFocus);
    assert_eq!(
        AwaitingFocus.advance(OnTarget),
        Stabilizing { confirmed: 1 }
    );
    assert_eq!(Stabilizing { confirmed: 1 }.advance(OnTarget), Settled);
    assert_eq!(
        Stabilizing { confirmed: 1 }.advance(OffTarget),
        AwaitingFocus
    );
    assert_eq!(Settled.advance(OffTarget), Settled);
    assert_eq!(Settled.advance(OnTarget), Settled);
}
