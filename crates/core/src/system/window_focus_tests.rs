use super::*;

#[test]
fn focus_settle_defaults_match_production() {
    assert_eq!(FOCUS_SETTLE_TIMEOUT_MS, 750);
    assert_eq!(FOCUS_POLL_INTERVAL_MS, 50);
}
