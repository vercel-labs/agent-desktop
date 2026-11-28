use super::stub::Stub;
use crate::adapter::minimal_ref_entry;
use crate::commands::status::execute_with_report_with_context;
use crate::refs::{RefMap, refs_store::RefStore, refs_test_support::HomeGuard};
use crate::session::{ArtifactsMode, StartSessionOptions, start_session};
use crate::{CommandContext, PermissionReport, PermissionState};
use serde_json::Value;

fn status(context: &CommandContext) -> Value {
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Denied {
            suggestion: "Enable it".into(),
        },
        automation: PermissionState::NotRequired,
    };
    execute_with_report_with_context(&Stub::default(), &report, context).unwrap()
}

golden_test!(
    status_without_session_or_snapshot_shows_null_rows,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-status_without_session_or_snapshot_shows_null_rows.json"
    ),
    &["platform", "version", "state_root"],
    {
        let home = HomeGuard::new();
        let value = status(&CommandContext::default());
        assert_eq!(
            value["state_root"],
            home.path()
                .join(".agent-desktop")
                .to_string_lossy()
                .as_ref()
        );
        value
    }
);

golden_test!(
    status_with_a_session_and_a_snapshot,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-status_with_a_session_and_a_snapshot.json"
    ),
    &[
        "platform",
        "version",
        "state_root",
        "session_id",
        "snapshot_id"
    ],
    {
        let _home = HomeGuard::new();
        let options = StartSessionOptions {
            artifacts: ArtifactsMode::Full,
            ..Default::default()
        };
        let id = start_session(options).unwrap().id;
        let mut refs = RefMap::new();
        refs.allocate(minimal_ref_entry("button", Some("Send")));
        RefStore::for_session(Some(&id))
            .unwrap()
            .save_new_snapshot(&refs)
            .unwrap();
        status(&CommandContext::new(Some(id), None, false).unwrap())
    }
);
