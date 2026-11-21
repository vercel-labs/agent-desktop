use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps, WindowFilter};
use crate::commands::dismiss_all_notifications::{self, DismissAllNotificationsArgs};
use crate::commands::dismiss_notification::{self, DismissNotificationArgs};
use crate::commands::focus_window::{self, FocusWindowArgs};
use crate::commands::launch::{self, LaunchArgs};
use crate::commands::notification_action::{self, NotificationActionArgs};
use crate::commands::open_system_surface::{self, OpenSystemSurfaceArgs};
use crate::commands::tests::golden::assert_golden;
use crate::context::CommandContext;
use crate::system::{launch_options::LaunchOptions, launch_result::LaunchResult};
use crate::{
    ActionResult, AdapterError, DismissAllNotificationsRequest, DismissNotificationRequest,
    InteractionLease, InteractionPolicy, NotificationActionRequest, NotificationInfo,
    ProcessIdentity, SnapshotSurface, WindowInfo,
};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;

#[derive(Default)]
struct Stub {
    failures: Vec<&'static str>,
    window: Option<WindowInfo>,
    launched: Option<LaunchResult>,
}

impl ObservationOps for Stub {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self.window.clone().into_iter().collect())
    }

    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _deadline: crate::Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Ok(query.answer(Vec::new()))
    }
}
impl ActionOps for Stub {}
impl InputOps for Stub {}

impl SystemOps for Stub {
    crate::adapter::guarded_interaction_lease!();

    fn dismiss_notification(
        &self,
        _request: DismissNotificationRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<NotificationInfo, AdapterError> {
        Ok(notification())
    }

    fn dismiss_all_notifications(
        &self,
        _request: DismissAllNotificationsRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
        Ok((
            vec![notification()],
            self.failures.iter().map(ToString::to_string).collect(),
        ))
    }

    fn notification_action(
        &self,
        _request: NotificationActionRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        let step = json!({"label": "press", "outcome": "succeeded", "mechanism": "semantic_api", "verified": true});
        let result = json!({
            "action": "click",
            "disposition": {"delivery": "delivered_verified", "retry": "unsafe"},
            "steps": [step],
            "surfaces": [{"id": "s-1", "type": "sheet", "title": "Reply", "item_count": 2, "unclassified": ["menu"]}],
            "post_state": {"role": "button", "value": "Reply", "enabled": true},
            "details": {"notification_index": 1},
        });
        Ok(serde_json::from_value(result).unwrap())
    }

    fn launch_app(
        &self,
        _id: &str,
        options: &LaunchOptions,
        _lease: &InteractionLease,
    ) -> Result<LaunchResult, AdapterError> {
        if let Some(port) = options.cdp_port {
            serve_devtools_endpoint(port);
        }
        Ok(self.launched.clone().unwrap())
    }

    fn process_owns_tcp_listener(
        &self,
        _process: ProcessIdentity,
        _port: u16,
        _deadline: crate::Deadline,
    ) -> Result<bool, AdapterError> {
        Ok(true)
    }

    fn open_system_surface(
        &self,
        _surface: SnapshotSurface,
        _policy: InteractionPolicy,
        _lease: &InteractionLease,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(self.window.clone().unwrap())
    }

    fn focus_window(
        &self,
        _window: &WindowInfo,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    fn focused_window(
        &self,
        _deadline: crate::Deadline,
    ) -> Result<Option<WindowInfo>, AdapterError> {
        let mut focused = self.window.clone().unwrap();
        focused.state.is_focused = true;
        Ok(Some(focused))
    }
}

fn serve_devtools_endpoint(port: u16) {
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("reserved port");
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.read(&mut [0_u8; 1024]);
            let socket = format!("ws://127.0.0.1:{port}/devtools/browser/x");
            let body =
                json!({"Browser": "Fixture/1.0", "webSocketDebuggerUrl": socket}).to_string();
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
            let _ = stream.write_all(format!("{head}{body}").as_bytes());
        }
    });
}

fn notification() -> NotificationInfo {
    NotificationInfo {
        index: 1,
        app_name: "Mail".into(),
        title: "New message".into(),
        body: Some("Lunch at noon?".into()),
        actions: vec!["Reply".into()],
    }
}

fn headed() -> CommandContext {
    CommandContext::default().with_headed(true)
}

fn window_json(id: &str) -> Value {
    json!({
        "id": id, "title": "Action center", "app_name": "Shell", "pid": 99, "process_instance": "instance-99",
        "bounds": {"x": 1140.0, "y": 60.0, "width": 380.0, "height": 640.0},
        "is_focused": false, "accessible": true, "minimized": false, "visible": true,
    })
}

fn windowed(id: &str) -> Stub {
    Stub {
        window: serde_json::from_value(window_json(id)).unwrap(),
        ..Stub::default()
    }
}

fn open(surface: SnapshotSurface, stub: &Stub) -> Value {
    open_system_surface::execute(OpenSystemSurfaceArgs { surface }, stub, &headed()).unwrap()
}

fn launch(result: Value, options: LaunchOptions) -> Value {
    let stub = Stub {
        launched: serde_json::from_value(result).unwrap(),
        ..Stub::default()
    };
    let args = LaunchArgs {
        app: "Fixture".into(),
        options,
    };
    launch::execute(args, &stub, InteractionPolicy::headed()).unwrap()
}

#[test]
fn dismiss_notification_reports_the_dismissed_notification() {
    let args = DismissNotificationArgs {
        index: 1,
        app: None,
        expected_app: Some("Mail".into()),
        expected_title: None,
    };
    let v = dismiss_notification::execute(args, &Stub::default(), &headed()).unwrap();
    const G: &str =
        include_str!("../../../../../../tests/fixtures/contract/lifecycle-dismiss-one.json");
    assert_golden(v, G, &[]);
}

#[test]
fn dismiss_all_notifications_adds_failures_only_when_there_are_some() {
    let run = |failures: Vec<&'static str>| {
        let args = DismissAllNotificationsArgs { app: None };
        let stub = Stub {
            failures,
            ..Stub::default()
        };
        dismiss_all_notifications::execute(args, &stub, &headed()).unwrap()
    };
    let v = json!([run(vec![]), run(vec!["Mail: no close action"])]);
    const G: &str =
        include_str!("../../../../../../tests/fixtures/contract/lifecycle-dismiss-all.json");
    assert_golden(v, G, &[]);
}

#[test]
fn notification_action_reports_the_full_action_result() {
    let args = NotificationActionArgs {
        index: 1,
        action: "Reply".into(),
        expected_app: Some("Mail".into()),
        expected_title: None,
    };
    let v = notification_action::execute(args, &Stub::default(), &headed()).unwrap();
    const G: &str =
        include_str!("../../../../../../tests/fixtures/contract/lifecycle-notif-action.json");
    assert_golden(v, G, &[]);
}

#[test]
fn launch_with_cdp_reports_instance_window_renderer_endpoint_and_connect_hint() {
    let result = json!({"app": "Fixture", "pid": 42, "process_instance": "42:1", "renderer": "chromium", "window": window_json("w-1")});
    let options = LaunchOptions {
        cdp_port: Some(0),
        ..LaunchOptions::default()
    };
    let value = launch(result, options);
    let port = value["cdp"]["port"].as_u64().unwrap().to_string();
    let masked = value
        .to_string()
        .replace(&format!("\"port\":{port}"), "\"port\":\"<port>\"")
        .replace(&port, "<port>");
    let v = serde_json::from_str(&masked).unwrap();
    const G: &str =
        include_str!("../../../../../../tests/fixtures/contract/lifecycle-launch-cdp.json");
    assert_golden(v, G, &[]);
}

#[test]
fn launch_without_facts_is_bare_and_a_chromium_app_without_cdp_gets_the_hint() {
    let bare = json!({"app": "Fixture", "pid": 42});
    let chromium = json!({"app": "Fixture", "pid": 42, "renderer": "chromium"});
    let v = json!([
        launch(bare, LaunchOptions::default()),
        launch(chromium, LaunchOptions::default())
    ]);
    const G: &str =
        include_str!("../../../../../../tests/fixtures/contract/lifecycle-launch-hints.json");
    assert_golden(v, G, &[]);
}

#[test]
fn open_system_surface_reports_the_window_only_for_a_window_and_spells_kinds_for_the_cli() {
    let kinds = [SnapshotSurface::SystemTray, SnapshotSurface::ActionCenter];
    let spellings = kinds.map(|kind| open(kind, &windowed(""))["surface"].clone());
    let v = json!({
        "with_window": open(SnapshotSurface::ActionCenter, &windowed("w-4242")),
        "without_window": open(SnapshotSurface::Dock, &windowed("")),
        "spellings": spellings,
    });
    const G: &str =
        include_str!("../../../../../../tests/fixtures/contract/lifecycle-surface.json");
    assert_golden(v, G, &[]);
}

#[test]
fn focus_window_reports_the_window_once_the_os_confirms_focus() {
    let args = FocusWindowArgs {
        window_id: Some("w-4242".into()),
        app: None,
        title: None,
    };
    let v = focus_window::execute(args, &windowed("w-4242"), InteractionPolicy::headed()).unwrap();
    const G: &str =
        include_str!("../../../../../../tests/fixtures/contract/lifecycle-focus-window.json");
    assert_golden(v, G, &[]);
}
