use crate::system::cdp_endpoint::CdpEndpoint;
use crate::system::launch_result::LaunchResult;
use crate::system::process_state::ProcessState;
use crate::*;
use serde_json::json;

#[test]
fn rect_and_display_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-rect-display.json"),
        json!({
            "rect": Rect { x: -12.5, y: 40.0, width: 1280.25, height: 720.0 },
            "non_finite": Rect { x: f64::NAN, y: f64::INFINITY, width: f64::NEG_INFINITY, height: 0.0 },
            "display": DisplayInfo { id: "display-2".into(), bounds: Rect { x: -1920.0, y: 0.0, width: 1920.0, height: 1080.0 }, is_primary: false, scale: 1.5 },
        })
    );
}

#[test]
fn window_info_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-window-info.json"),
        json!({
            "full": WindowInfo { id: "w-12".into(), title: "Untitled".into(), app: "Fixture".into(), pid: ProcessId::new(4242), process_instance: Some("start-time:1700000000".into()), bounds: Some(Rect { x: 10.0, y: 20.0, width: 800.0, height: 600.0 }), state: WindowState { is_focused: true, accessible: false, minimized: Some(true), visible: Some(false) } },
            "minimal": WindowInfo { id: "w-1".into(), title: String::new(), app: "Fixture".into(), pid: ProcessId::new(7), process_instance: None, bounds: None, state: WindowState::default() },
            "process_owner": WindowInfo::for_process(&AppInfo { name: "Fixture".into(), pid: ProcessId::new(4242), bundle_id: None, process_instance: Some("start-time:1700000000".into()), presentation: None, aliases: Vec::new() }),
        })
    );
}

#[test]
fn app_info_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-app-info.json"),
        json!({
            "full": AppInfo { name: "Fixture".into(), pid: ProcessId::new(4242), bundle_id: Some("com.example.fixture".into()), process_instance: Some("start-time:1700000000".into()), presentation: Some(AppPresentation::Foreground), aliases: vec!["fixture-bin".into()] },
            "minimal": AppInfo { name: "Fixture".into(), pid: ProcessId::new(7), bundle_id: None, process_instance: None, presentation: None, aliases: Vec::new() },
            "background": AppPresentation::Background,
        })
    );
}

#[test]
fn launch_result_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-launch-result.json"),
        json!({
            "full": LaunchResult { app: "Fixture".into(), pid: ProcessId::new(4242), process_instance: Some("start-time:1700000000".into()), window: Some(WindowInfo { id: "w-3".into(), title: "Main".into(), app: "Fixture".into(), pid: ProcessId::new(4242), process_instance: None, bounds: None, state: WindowState::default() }), guidance: LaunchGuidance { cdp: Some(CdpEndpoint { port: 9222, http_endpoint: "http://127.0.0.1:9222".into(), websocket_url: "ws://127.0.0.1:9222/devtools/browser/abc".into(), product: Some("Chrome/130.0.0.0".into()) }), renderer: Some(RendererKind::Chromium), suggestion: Some("Drive it through DevTools".into()) }, disposition: DeliverySemantics::delivered_verified() },
            "minimal": LaunchResult { app: "Fixture".into(), pid: ProcessId::new(7), process_instance: None, window: None, guidance: LaunchGuidance::default(), disposition: DeliverySemantics::unknown() },
            "cdp_without_product": CdpEndpoint { port: 9222, http_endpoint: "http://127.0.0.1:9222".into(), websocket_url: "ws://127.0.0.1:9222/devtools/browser/abc".into(), product: None },
        })
    );
}

#[test]
fn process_state_variants() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-process-state.json"),
        json!([
            ProcessState::Running,
            ProcessState::Exited { code: Some(3) },
            ProcessState::Exited { code: None },
            ProcessState::Crashed { signal_or_code: 11 },
            ProcessState::Unresponsive,
        ])
    );
}

#[test]
fn operation_target_variants() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-operation-target.json"),
        json!([
            OperationTarget::Window {
                id: "w-12".into(),
                title: "Untitled".into(),
                process: ProcessIdentity::new(4242_u32, "start-time:1700000000")
            },
            OperationTarget::App {
                name: "Fixture".into(),
                process: ProcessIdentity::new(7_u32, "")
            },
            OperationTarget::Pointer {
                point: Point { x: 10.5, y: 20.0 }
            },
            OperationTarget::Drag {
                from: Point { x: 1.0, y: 2.0 },
                to: Point {
                    x: 300.0,
                    y: 400.25
                }
            },
            OperationTarget::Clipboard,
        ])
    );
}
