use super::imp::wait_until_on_screen;
use crate::system::test_support::test_time::deadline;
use agent_desktop_core::ErrorCode;

#[test]
fn an_exhausted_deadline_after_the_realize_wait_is_a_timeout() {
    let error = wait_until_on_screen(deadline(0), || false).expect_err("timeout");
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn a_realize_wait_with_the_item_on_screen_and_time_left_passes() {
    wait_until_on_screen(deadline(5_000), || true).expect("live deadline");
}
