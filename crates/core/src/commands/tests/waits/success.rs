use super::stub::{Stub, app, args, build, golden, mode, run, script, selector, signals, stable};
use super::stub::{window, with_buttons};
use crate::commands::wait::execute;
use crate::commands::wait_surface::SurfaceWait;
use crate::refs::refs_test_support::HomeGuard;
use crate::{CommandContext, NotificationInfo, SignalBaseline};
use serde_json::{Value, json};

#[test]
fn sleep_window_and_menu_waits_report_their_data() {
    let report = build(
        json!({"id": "w-7", "title": "Quarterly report.txt", "app_name": "TextEdit",
        "pid": 42, "process_instance": "test-instance", "is_focused": true, "minimized": false,
        "visible": true, "bounds": {"x": 10.0, "y": 20.0, "width": 640.0, "height": 480.0}}),
    );
    let stub = Stub {
        windows: script(vec![Ok(vec![report])]),
        ..Default::default()
    };

    let data = json!({
        "menu": run(&stub, mode!(surface, SurfaceWait::Menu), Some("TextEdit"), 2_000).unwrap(),
        "sleep": run(&stub, mode!(ms, 5u64), None, 5_000).unwrap(),
        "window": run(&stub, mode!(window, "report"), None, 2_000).unwrap(),
    });

    golden!(
        data,
        include_str!("../../../../../../tests/fixtures/contract/waits-simple.json")
    );
}

#[test]
fn notification_wait_reports_the_first_new_notification() {
    let mail = |index: usize, title: &str| -> NotificationInfo {
        build(json!({"index": index, "app_name": "Mail", "title": title,
            "body": "Lunch at noon?", "actions": ["Reply", "Archive"]}))
    };
    let stub = Stub {
        notifications: script(vec![
            Ok(vec![mail(0, "Old mail")]),
            Ok(vec![mail(0, "Old mail"), mail(1, "New mail")]),
        ]),
        ..Default::default()
    };
    let notification = mode!(surface, SurfaceWait::Notification);

    golden!(
        run(&stub, notification, None, 5_000).unwrap(),
        include_str!("../../../../../../tests/fixtures/contract/waits-notification.json")
    );
}

#[test]
fn text_wait_reports_the_qualified_ref_and_the_count() {
    let _home = HomeGuard::new();
    let stub = with_buttons(&["Save", "Save as"]);
    let counted = |text: &str, count: usize| {
        let mut wait = args(mode!(text, text), Some("TextEdit"), 2_000);
        wait.predicate.count = Some(count);
        stable(
            execute(wait, &stub, &CommandContext::default()).unwrap(),
            &[],
        )
    };

    let data = json!({"matched": counted("Save", 2), "zero_count": counted("Missing", 0)});

    golden!(
        data,
        include_str!("../../../../../../tests/fixtures/contract/waits-text.json")
    );
}

#[test]
fn selector_wait_reports_the_snapshot_and_absence() {
    let _home = HomeGuard::new();
    let stub = with_buttons(&["Save", "Cancel"]);
    let matched = stable(selector(&stub, "button:Save", false, 2_000).unwrap(), &[]);
    let gone = selector(&stub, "button:Spinner", true, 2_000).unwrap();

    golden!(
        json!({"gone": gone, "matched": matched}),
        include_str!("../../../../../../tests/fixtures/contract/waits-selector.json")
    );
}

#[test]
fn event_wait_reports_the_window_and_app_events() {
    let event = |name: &str, current: SignalBaseline| -> Value {
        let stub = Stub {
            signals: current,
            ..Default::default()
        };
        let seed = signals(vec![], vec![]);
        let context = CommandContext::default().with_event_baseline(Some(Ok(seed)));
        execute(args(mode!(event, name), None, 2_000), &stub, &context).unwrap()
    };

    let data = json!({
        "app_launched": event("app-launched", signals(vec![], vec![app()])),
        "window_opened": event("window-opened", signals(vec![window("w-9", "Untitled")], vec![])),
    });

    golden!(
        data,
        include_str!("../../../../../../tests/fixtures/contract/waits-event.json")
    );
}
