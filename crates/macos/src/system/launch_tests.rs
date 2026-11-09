use super::*;
use crate::system::launch_frontmost::{frontmost_is, owner_is_target};

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
fn exact_native_launch_rejects_a_working_directory() {
    let options = LaunchOptions {
        cwd: Some(std::path::PathBuf::from("/tmp")),
        ..Default::default()
    };

    let error = validate_launch_options(&options).expect_err("cwd is unsupported");

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}

#[test]
fn launch_options_enforce_bounded_entry_counts() {
    let options = LaunchOptions {
        args: (0..=MAX_ARGUMENT_COUNT)
            .map(|index| index.to_string())
            .collect(),
        ..Default::default()
    };

    let error = validate_launch_options(&options).expect_err("too many args");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

#[test]
fn launch_options_enforce_a_bounded_text_budget() {
    let options = LaunchOptions {
        args: vec!["x".repeat(MAX_LAUNCH_TEXT_BYTES + 1)],
        ..Default::default()
    };

    let error = validate_launch_options(&options).expect_err("payload too large");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

fn own_pid() -> i32 {
    i32::try_from(std::process::id()).expect("process id fits in a pid_t")
}

#[test]
fn attaching_without_activation_reports_not_delivered() {
    let app = AppInfo {
        aliases: Vec::new(),
        name: "Fixture".into(),
        pid: agent_desktop_core::ProcessId::new(std::process::id()),
        bundle_id: None,
        process_instance: Some("instance".into()),
        presentation: None,
    };

    let result = result_from_app(&app, None);

    assert_eq!(result.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_fresh_instance_absent_from_the_baseline_reports_delivered_verified() {
    let target = LaunchTarget::New {
        baseline_instances: vec!["earlier".into()],
    };

    let result = result_from_launched(
        &(own_pid(), "fresh".into()),
        None,
        "Fixture",
        target_facts(&target),
    )
    .expect("the pid is in range");

    assert_eq!(result.disposition, DeliverySemantics::delivered_verified());
}

#[test]
fn a_frontmost_read_that_disagrees_with_the_instance_is_not_a_postcondition() {
    let deadline = Deadline::after(1_000).expect("a positive timeout builds a deadline");

    assert!(!frontmost_is(own_pid(), "not-the-live-instance", deadline));
}

fn frontmost_owner(
    pid: i32,
    launch_time: Option<f64>,
) -> super::super::app::workspace_apps::WindowOwner {
    super::super::app::workspace_apps::WindowOwner {
        pid,
        name: "Fixture".into(),
        bundle_id: None,
        executable: None,
        launch_time,
        activation_policy: super::super::app::workspace_apps::ActivationPolicy::Regular,
    }
}

fn own_instance() -> (i32, String, f64) {
    let pid = own_pid();
    let identity = super::super::process::process_identity::ProcessIdentity::capture(pid)
        .expect("process identity reads")
        .expect("the test process has an identity");
    (pid, identity.token(), identity.launch_time_seconds())
}

#[test]
fn the_frontmost_owner_is_the_target_only_on_an_exact_instance_match() {
    let (pid, token, launch_time) = own_instance();
    let owner = frontmost_owner(pid, Some(launch_time));

    assert!(owner_is_target(&owner, pid, &token));
    assert!(!owner_is_target(&owner, pid + 1, &token));
    assert!(!owner_is_target(
        &frontmost_owner(pid, Some(launch_time + 3_600.0)),
        pid,
        &token
    ));
    assert!(!owner_is_target(&frontmost_owner(pid, None), pid, &token));
}

#[test]
fn attaching_with_activation_reports_delivered_unverified() {
    let target = LaunchTarget::Existing {
        pid: own_pid(),
        process_instance: "running".into(),
    };

    let result = result_from_launched(
        &(own_pid(), "running".into()),
        None,
        "Fixture",
        target_facts(&target),
    )
    .expect("the pid is in range");

    assert_eq!(
        result.disposition,
        DeliverySemantics::delivered_unverified()
    );
}
