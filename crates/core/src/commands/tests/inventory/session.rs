use crate::commands::session::{SessionAction, execute};
use crate::refs::refs_test_support::HomeGuard;
use crate::session::{ArtifactsMode, SessionManifest, SessionTraceMode, write_manifest};
use serde_json::json;

fn manifest(id: &str, created_at: u64, ended_at: Option<u64>) -> SessionManifest {
    let ended = ended_at.is_some();
    SessionManifest {
        id: id.into(),
        name: ended.then(|| "ended run".into()),
        created_at,
        ended_at,
        trace: if ended {
            SessionTraceMode::On
        } else {
            SessionTraceMode::Off
        },
        artifacts: if ended {
            ArtifactsMode::Full
        } else {
            ArtifactsMode::Events
        },
        cursor_overlay: crate::CursorOverlayConfig::default(),
    }
}

golden_test!(
    session_start_returns_the_id_and_the_activation,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-session_start_returns_the_id_and_the_activation.json"
    ),
    &["session_id", "created_at", "next", "value"],
    {
        let _home = HomeGuard::new();
        let start = SessionAction::Start {
            name: Some("checkout".into()),
            no_trace: false,
            screenshots: true,
        };
        let value = execute(start).unwrap();
        let id = value["session_id"].as_str().unwrap().to_string();
        assert!(value["next"].as_str().unwrap().contains(&id));
        assert_eq!(value["activation"]["value"], id.as_str());
        value
    }
);

golden_test!(
    session_list_end_and_gc,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-session_list_end_and_gc.json"
    ),
    &[],
    {
        let _home = HomeGuard::new();
        write_manifest(&manifest("ended-run", 2000, Some(5000))).unwrap();
        write_manifest(&manifest("open-run", 1000, None)).unwrap();
        let list = execute(SessionAction::List).unwrap();
        let end = execute(SessionAction::End {
            id: "ended-run".into(),
        })
        .unwrap();
        let gc = execute(SessionAction::Gc {
            older_than_secs: None,
            ended_only: true,
        })
        .unwrap();
        json!([list, end, gc])
    }
);
