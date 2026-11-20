use super::*;

#[test]
fn no_attach_requests_a_fresh_application_instance() {
    let options = LaunchOptions {
        attach_if_running: false,
        ..Default::default()
    };

    assert!(crate::system::launch_workspace::creates_new_instance(
        &options
    ));
}

#[test]
fn default_launch_allows_attaching_to_a_running_instance() {
    let options = LaunchOptions::default();

    assert!(!crate::system::launch_workspace::creates_new_instance(
        &options
    ));
}

#[test]
fn validates_safe_names_and_bundle_identifiers() {
    assert!(validate_app_identifier("Safari").is_ok());
    assert!(validate_app_identifier("com.apple.Safari").is_ok());
}

#[test]
fn rejects_paths_and_unsafe_bundle_identifiers() {
    for identifier in [
        "../Evil",
        "/abs/path",
        "Foo/Bar",
        "bad\0name",
        "bad\nname",
        "com.apple.$evil",
    ] {
        let error = validate_app_identifier(identifier).expect_err("unsafe identifier");
        assert_eq!(error.code, ErrorCode::InvalidArgs);
        assert!(!error.message.contains(identifier));
        assert_eq!(error.details.expect("details")["app_name"], identifier);
    }
}

#[test]
fn waiting_ends_only_after_a_completed_startup_plus_its_grace() {
    let now = std::time::Instant::now();

    assert!(!grace_over(None, now));
    assert!(!grace_over(now.checked_add(STARTUP_GRACE), now));
    assert!(grace_over(Some(now), now));
}

#[test]
fn app_unresponsive_and_timeout_window_reads_are_not_ready_yet() {
    assert!(window_read_is_not_ready_yet(
        &AdapterError::app_unresponsive("Numbers")
    ));
    assert!(window_read_is_not_ready_yet(&AdapterError::timeout("busy")));
}

#[test]
fn every_other_error_code_ends_the_wait_immediately() {
    assert!(!window_read_is_not_ready_yet(&AdapterError::stale_ref(
        "@e1"
    )));
    assert!(!window_read_is_not_ready_yet(
        &AdapterError::permission_denied()
    ));
}
