use super::stub::with_buttons;
use super::stub::{Stub, args, golden, mode, payload, run, script, selector, stable, window};
use crate::commands::wait::execute;
use crate::commands::wait_surface::SurfaceWait;
use crate::refs::refs_test_support::HomeGuard;
use crate::{AdapterError, CommandContext, ErrorCode};
use serde_json::json;

#[test]
fn wait_timeouts_report_their_evidence() {
    let _home = HomeGuard::new();
    let rebuilding = || {
        let error = AdapterError::new(ErrorCode::WindowNotFound, "The window list is rebuilding");
        Stub {
            windows: script(vec![Err(error), Ok(vec![window("w-1", "Inbox")])]),
            ..Default::default()
        }
    };
    let menu_stub = Stub {
        menu_times_out: true,
        ..Default::default()
    };
    let unreachable = Stub {
        windows: script(vec![Err(AdapterError::new(
            ErrorCode::AppNotFound,
            "app missing",
        ))]),
        ..Default::default()
    };
    let seed_error = AdapterError::app_unresponsive("macOS inventory")
        .with_details(json!({"kind": "inventory_sources"}));
    let context = CommandContext::default().with_event_baseline(Some(Err(seed_error)));
    let event = args(mode!(event, "window-opened"), None, 100);
    let (menu, notification) = (SurfaceWait::Menu, SurfaceWait::Notification);

    let data = json!({
        "event": payload(execute(event, &Stub::default(), &context)),
        "menu": payload(run(&menu_stub, mode!(surface, menu), Some("TextEdit"), 2_000)),
        "notification": payload(run(&Stub::default(), mode!(surface, notification), None, 100)),
        "selector_last_error": payload(selector(&unreachable, "button:Save", false, 100)),
        "selector_snapshot": stable(payload(selector(&with_buttons(&["Cancel"]), "button:Save", false, 100)), &[]),
        "text": payload(run(&rebuilding(), mode!(text, "Quarterly"), Some("TextEdit"), 100)),
        "window": payload(run(&rebuilding(), mode!(window, "Quarterly"), Some("TextEdit"), 250)),
    });

    golden!(
        data,
        include_str!("../../../../../../tests/fixtures/contract/waits-timeouts.json")
    );
}
