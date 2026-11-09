use super::*;

#[test]
fn expired_permission_deadline_fails_without_native_calls() {
    let error = report(Deadline::after(0).unwrap()).unwrap_err();

    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
}

#[test]
fn automation_probe_maps_without_a_prompting_state() {
    assert_eq!(map_automation_probe(0), PermissionState::Granted);
    assert!(matches!(
        map_automation_probe(-1743),
        PermissionState::Denied { .. }
    ));
    assert_eq!(map_automation_probe(-1744), PermissionState::Unknown);
    assert_eq!(map_automation_probe(-600), PermissionState::Unknown);
}

#[test]
fn post_helper_preflight_is_the_authoritative_permission_state() {
    let mut requested = Vec::new();
    let report = request_report_with(
        Deadline::after(1_000).unwrap(),
        |operation, _| {
            requested.push(operation);
            Ok(false)
        },
        |_| {
            Ok(PermissionReport {
                accessibility: PermissionState::Granted,
                screen_recording: PermissionState::Granted,
                automation: PermissionState::Unknown,
            })
        },
    )
    .unwrap();

    assert_eq!(requested.len(), 3);
    assert_eq!(report.accessibility, PermissionState::Granted);
    assert_eq!(report.screen_recording, PermissionState::Granted);
    assert_eq!(report.automation, PermissionState::Unknown);
}
