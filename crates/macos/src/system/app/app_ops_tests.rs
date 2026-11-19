use super::*;

#[test]
fn protected_processes_match_display_and_bundle_forms() {
    assert!(is_protected_process("Finder"));
    assert!(is_protected_process("Dock"));
    assert!(is_protected_process("com.apple.dock"));
    assert!(is_protected_process("WindowServer"));
    assert!(is_protected_process("loginwindow"));
}

#[test]
fn ordinary_apps_are_not_protected() {
    assert!(!is_protected_process("TextEdit"));
    assert!(!is_protected_process("Safari"));
    assert!(!is_protected_process("com.company.MyApp"));
}

#[test]
fn lookalike_names_containing_protected_substrings_stay_closable() {
    assert!(!is_protected_process("Docker"));
    assert!(!is_protected_process("Docker Desktop"));
    assert!(!is_protected_process("com.docker.docker-desktop"));
    assert!(!is_protected_process("FinderSync"));
    assert!(!is_protected_process("PathFinder"));
    assert!(!is_protected_process("launchdarkly-agent"));
}

#[test]
fn adapter_guard_refuses_protected_processes_with_the_cli_contract() {
    let err = ensure_not_protected("loginwindow").unwrap_err();

    assert_eq!(err.code, agent_desktop_core::ErrorCode::InvalidArgs);
    assert!(err.message.contains("protected"));
    assert!(err.suggestion.is_some());
    assert!(ensure_not_protected("TextEdit").is_ok());
}

#[test]
fn native_termination_rejection_does_not_blame_the_target_application() {
    let error = termination_request_not_accepted("Fixture", 42, false);

    assert_eq!(error.code, agent_desktop_core::ErrorCode::ActionFailed);
    assert!(
        error
            .message
            .contains("native termination API did not accept")
    );
    assert!(!error.message.contains("App 'Fixture' rejected"));
    assert_eq!(error.details.as_ref().unwrap()["pid"], 42);
    assert_eq!(error.details.as_ref().unwrap()["force"], false);
}

#[test]
fn dialog_titles_collects_alerts_and_sheets_in_surface_order() {
    use agent_desktop_core::SurfaceInfo;

    let surface = |id: &str, kind: &str, title: Option<&str>| SurfaceInfo {
        id: id.into(),
        kind: kind.into(),
        title: title.map(str::to_string),
        item_count: None,
        unclassified: Vec::new(),
    };
    let surfaces = vec![
        surface("focused-window", "alert", Some("Review changes?")),
        surface("focused-window/children/0", "sheet", None),
        surface("focused-window/children/1", "popover", Some("Not a prompt")),
        surface("menubar", "menu", Some("File")),
    ];

    assert_eq!(
        dialog_titles(&surfaces),
        vec!["Review changes?".to_string(), String::new()]
    );
    assert!(dialog_titles(&surfaces[2..]).is_empty());
}

#[test]
fn dialog_probe_budget_uses_the_close_deadline_up_to_one_standard_operation() {
    assert_eq!(
        dialog_probe_budget_ms(5_000),
        agent_desktop_core::DEFAULT_OPERATION_TIMEOUT_MS
    );
    assert_eq!(
        dialog_probe_budget_ms(30_000),
        agent_desktop_core::DEFAULT_OPERATION_TIMEOUT_MS
    );
    assert_eq!(dialog_probe_budget_ms(1_000), 1_000);
    assert_eq!(dialog_probe_budget_ms(0), 0);
}

#[test]
fn probe_outcome_reports_titles_only_when_a_prompt_is_listed() {
    use agent_desktop_core::SurfaceInfo;

    let surface = |id: &str, kind: &str, title: Option<&str>| SurfaceInfo {
        id: id.into(),
        kind: kind.into(),
        title: title.map(str::to_string),
        item_count: None,
        unclassified: Vec::new(),
    };
    let listed = probe_outcome_from(Ok(vec![
        surface("focused-window", "sheet", Some("save")),
        surface("focused-window/children/0", "menu", Some("File")),
    ]));
    assert!(matches!(listed, DialogProbe::Found(titles) if titles == vec!["save".to_string()]));
    let unlisted = probe_outcome_from(Ok(vec![surface(
        "focused-window",
        "window",
        Some("Untitled"),
    )]));
    assert!(matches!(unlisted, DialogProbe::Absent));
}

#[test]
fn probe_outcome_reports_a_timed_out_probe_instead_of_guessing() {
    let outcome = probe_outcome_from(Err(AdapterError::timeout(
        "Accessibility surface resolution exceeded its absolute deadline",
    )));

    assert!(matches!(outcome, DialogProbe::TimedOut));
}

#[test]
fn probe_outcome_treats_other_probe_failures_as_no_prompt() {
    let outcome = probe_outcome_from(Err(AdapterError::new(
        agent_desktop_core::ErrorCode::ActionFailed,
        "accessibility is unavailable",
    )));

    assert!(matches!(outcome, DialogProbe::Absent));
}

#[test]
fn probe_timeout_error_names_the_probe_and_points_at_list_surfaces() {
    let deadline = Deadline::detached_after(5_000).expect("deadline");
    let error = dialog_probe_timeout_error(deadline, "TextEdit", 42, false);

    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
    assert!(
        error
            .suggestion
            .as_deref()
            .unwrap()
            .contains("list-surfaces")
    );
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["app"], "TextEdit");
    assert_eq!(details["pid"], 42);
    assert_eq!(details["force"], false);
    assert_eq!(details["dialog_probe"], "timed_out");
}

#[test]
fn adapter_close_without_a_process_instance_keeps_the_invalid_args_contract() {
    use agent_desktop_core::{InteractionLease, SystemOps};

    let app = AppInfo {
        name: "TextEdit".into(),
        pid: agent_desktop_core::ProcessId::new(42),
        bundle_id: None,
        process_instance: None,
        presentation: None,
    };
    let deadline = Deadline::detached_after(1_000).expect("deadline");
    let lease = InteractionLease::guarded(deadline, ()).expect("lease");

    let error = crate::adapter::MacOSAdapter::new()
        .close_app(&app, false, &lease)
        .unwrap_err();

    assert_eq!(error.code, agent_desktop_core::ErrorCode::InvalidArgs);
    assert!(error.message.contains("process instance"));
}
