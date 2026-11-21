use super::*;

/// An incomplete observation is recorded as the wait's last error under the
/// locator's own kind and cause, so a timed-out wait says why it could not
/// prove the selector.
#[test]
fn an_incomplete_observation_is_recorded_with_the_locators_kind_and_cause() {
    let mut stats = crate::LocatorStats::default();
    stats.reads.health.native_read_failures = 2;
    let resolution = crate::LocatorResolution {
        matches: Vec::new(),
        refmap: None,
        stats,
        meta: crate::locator::live_locator::LocatorResolutionMeta {
            total_matches: 0,
            complete: false,
            selection_complete: false,
            truncated: false,
            roles_present: Vec::new(),
        },
    };

    let recorded = incomplete_poll_json(&crate::incomplete_locator_error(&resolution));

    assert_eq!(recorded["code"], "TIMEOUT");
    assert_eq!(recorded["kind"], "locator_incomplete");
    assert_eq!(recorded["cause"], "unreadable_elements");
    assert!(
        recorded["message"]
            .as_str()
            .is_some_and(|message| message.contains("2 accessibility reads failed"))
    );
}
