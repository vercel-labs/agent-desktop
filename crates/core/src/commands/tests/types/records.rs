use crate::locator::live_locator::LocatorMatchData;
use crate::*;
use serde_json::json;

#[test]
fn notification_info_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-notification-info.json"),
        json!({
            "full": NotificationInfo { index: 2, app_name: "Mail".into(), title: "New message".into(), body: Some("Lunch at noon?".into()), actions: vec!["Reply".into()] },
            "minimal": NotificationInfo { index: 0, app_name: "Mail".into(), title: String::new(), body: None, actions: Vec::new() },
        })
    );
}

#[test]
fn ui_event_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-ui-event.json"),
        json!({
            "full": UiEvent { kind: EventKind::SurfaceAppeared { surface: SnapshotSurface::Sheet }, window_id: Some("w-9".into()), title: Some("Preferences".into()), app: Some("Fixture".into()), pid: Some(ProcessId::new(4242)) },
            "minimal": UiEvent { kind: EventKind::FocusChangedWindow, window_id: None, title: None, app: None, pid: None },
            "kinds": [EventKind::WindowOpened, EventKind::WindowClosed, EventKind::AppLaunched, EventKind::AppTerminated, EventKind::SurfaceDismissed { surface: SnapshotSurface::SystemTray }],
        })
    );
}

#[test]
fn locator_match_data_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-locator-match-data.json"),
        json!({
            "full": LocatorMatchData { ref_id: Some("@s8f3k2p9:e4".into()), role: "button".into(), name: "Save".into(), value: Some("on".into()), states: vec!["focused".into()], interactive: true, path: vec!["window".into()] },
            "empty": LocatorMatchData { ref_id: None, role: "group".into(), name: String::new(), value: None, states: Vec::new(), interactive: false, path: Vec::new() },
        })
    );
}
