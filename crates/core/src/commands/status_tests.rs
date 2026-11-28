use super::*;
use crate::PermissionState;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};

struct DeniedAdapter;

impl ObservationOps for DeniedAdapter {}

impl ActionOps for DeniedAdapter {}

impl InputOps for DeniedAdapter {}

impl SystemOps for DeniedAdapter {
    fn permission_report(
        &self,
        _deadline: crate::Deadline,
    ) -> Result<PermissionReport, crate::AdapterError> {
        Ok(PermissionReport {
            accessibility: PermissionState::Denied {
                suggestion: "should not be used".into(),
            },
            screen_recording: PermissionState::Denied {
                suggestion: "should not be used".into(),
            },
            automation: PermissionState::Unknown,
        })
    }
}

#[test]
fn status_uses_precomputed_permission_report() {
    let _guard = crate::refs::refs_test_support::HomeGuard::new();
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };

    let value =
        execute_with_report_with_context(&DeniedAdapter, &report, &CommandContext::default())
            .unwrap();
    let permissions = value.get("permissions").unwrap();

    assert_eq!(permissions["accessibility"]["state"], "granted");
    assert_eq!(permissions["screen_recording"]["state"], "granted");
    assert_eq!(permissions["automation"]["state"], "not_required");
}

#[test]
fn status_reports_tracing_false_when_writer_failed() {
    let _guard = crate::refs::refs_test_support::HomeGuard::new();
    let session = crate::session::start_session(crate::session::StartSessionOptions {
        name: None,
        trace: crate::session::SessionTraceMode::On,
        ..Default::default()
    })
    .unwrap();
    let unopenable = std::env::temp_dir()
        .join("agent-desktop-status-nodir")
        .join("trace.jsonl");
    let context = CommandContext::new(Some(session.id), Some(unopenable), false).unwrap();
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };

    let value = execute_with_report_with_context(&DeniedAdapter, &report, &context).unwrap();

    assert_eq!(
        value["tracing"], false,
        "a failed trace writer must not report tracing:true"
    );
}

#[test]
fn status_reports_resolved_state_root_verbatim() {
    let guard = crate::refs::refs_test_support::HomeGuard::new();
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };

    let value =
        execute_with_report_with_context(&DeniedAdapter, &report, &CommandContext::default())
            .unwrap();

    let expected = guard.path().join(".agent-desktop");
    assert_eq!(value["state_root"], expected.to_string_lossy().as_ref());
}

#[test]
fn status_reports_state_root_privacy_from_the_installed_private_file_check() {
    struct RefusingRoot;
    impl crate::PrivateFileOps for RefusingRoot {
        fn ensure_private(&self, path: &std::path::Path) -> std::io::Result<()> {
            Err(crate::private_file_error(
                path,
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "unsafe state root"),
                "Choose a private state folder",
            ))
        }
    }
    let guard = crate::refs::refs_test_support::HomeGuard::new();
    let root = guard.path().join(".agent-desktop");
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };
    let status = || {
        execute_with_report_with_context(&DeniedAdapter, &report, &CommandContext::default())
            .unwrap()
    };
    assert_eq!(status()["state_root_privacy"], json!({ "ok": true }));
    let unhealthy = crate::storage::private_file_ops::with_test_ops_override(
        std::rc::Rc::new(RefusingRoot),
        status,
    );
    let privacy = &unhealthy["state_root_privacy"];
    assert_eq!(privacy["ok"], false);
    assert_eq!(privacy["error"]["code"], "PERM_DENIED");
    assert_eq!(privacy["error"]["details"]["path"], root.to_str().unwrap());
    assert_eq!(
        privacy["error"]["suggestion"],
        "Choose a private state folder"
    );
    assert_eq!(privacy["error"]["message"], "IO error: unsafe state root");
    assert!(unhealthy["snapshot_id"].is_null());
    assert_eq!(status()["state_root_privacy"], json!({ "ok": true }));
}

#[test]
fn status_surfaces_artifacts_mode_for_active_session() {
    let _guard = crate::refs::refs_test_support::HomeGuard::new();
    let session = crate::session::start_session(crate::session::StartSessionOptions {
        artifacts: crate::session::ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap();
    let context = CommandContext::new(Some(session.id.clone()), None, false).unwrap();
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };

    let value = execute_with_report_with_context(&DeniedAdapter, &report, &context).unwrap();

    assert_eq!(value["artifacts"], "full");
}
