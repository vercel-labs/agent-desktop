use super::*;
use agent_desktop_core::{ProcessId, Rect, WindowState};

fn window(width: f64, height: f64) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Doc".into(),
        app: "Editor".into(),
        pid: ProcessId::new(7),
        process_instance: Some("7:1".into()),
        bounds: Some(Rect {
            x: 10.0,
            y: 10.0,
            width,
            height,
        }),
        state: WindowState::default(),
    }
}

fn judged(read: Result<WindowInfo, AdapterError>) -> LaunchWindow {
    LaunchWindow::of_read(read, 7, "7:1").unwrap()
}

#[test]
fn an_app_with_several_windows_and_none_focused_has_no_window_to_report() {
    let several = judged(Err(AdapterError::ambiguous_target(
        "two windows, none focused",
    )));

    assert!(matches!(several, LaunchWindow::Unnamed));
    assert!(several.report().is_none());
}

#[test]
fn a_window_without_established_bounds_is_reported_without_bounds() {
    let unplaced = judged(Ok(window(0.0, 0.0)));

    assert!(matches!(unplaced, LaunchWindow::Unplaced(_)));
    let reported = unplaced.report().unwrap();
    assert_eq!(reported.id, "w-1");
    assert!(reported.bounds.is_none());
    assert!(matches!(
        judged(Ok(window(800.0, 600.0))),
        LaunchWindow::Placed(_)
    ));
}

#[test]
fn a_window_of_another_process_instance_is_an_error() {
    let error = LaunchWindow::of_read(Ok(window(800.0, 600.0)), 7, "7:2")
        .err()
        .unwrap();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}
