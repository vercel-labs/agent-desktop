use super::*;

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

fn running(pid: u32) -> AppInfo {
    AppInfo {
        name: "Google Chrome".into(),
        pid: agent_desktop_core::ProcessId::new(pid),
        bundle_id: None,
        process_instance: None,
        presentation: None,
    }
}

#[test]
fn attaching_rejects_arguments_the_running_app_would_never_receive() {
    let options = LaunchOptions {
        args: vec!["https://example.com".into()],
        ..Default::default()
    };

    let error = validate_attach_options(&options, &[running(741)])
        .expect_err("arguments cannot reach a running instance");

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.details.expect("details")["pid"], 741);
}

#[test]
fn attaching_rejects_environment_the_running_app_would_never_receive() {
    let options = LaunchOptions {
        env: [("KEY".to_owned(), "VALUE".to_owned())].into(),
        ..Default::default()
    };

    let error = validate_attach_options(&options, &[running(741)])
        .expect_err("environment cannot reach a running instance");

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}

#[test]
fn attaching_rejects_arguments_even_when_activating() {
    let options = LaunchOptions {
        args: vec!["https://example.com".into()],
        activate: true,
        ..Default::default()
    };

    assert!(validate_attach_options(&options, &[running(741)]).is_err());
}

#[test]
fn attaching_without_arguments_or_environment_is_allowed() {
    assert!(validate_attach_options(&LaunchOptions::default(), &[running(741)]).is_ok());
}

#[test]
fn arguments_are_allowed_when_nothing_is_running() {
    let options = LaunchOptions {
        args: vec!["https://example.com".into()],
        ..Default::default()
    };

    assert!(validate_attach_options(&options, &[]).is_ok());
}

#[test]
fn arguments_are_allowed_for_a_fresh_instance() {
    let options = LaunchOptions {
        args: vec!["https://example.com".into()],
        attach_if_running: false,
        ..Default::default()
    };

    assert!(validate_attach_options(&options, &[running(741)]).is_ok());
}

#[test]
fn several_running_instances_are_left_to_the_ambiguity_check() {
    let options = LaunchOptions {
        args: vec!["https://example.com".into()],
        ..Default::default()
    };

    assert!(validate_attach_options(&options, &[running(741), running(742)]).is_ok());
}
