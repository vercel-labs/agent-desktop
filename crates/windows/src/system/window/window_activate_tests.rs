use super::*;
use agent_desktop_core::{Deadline, InteractionLease, ProcessId, WindowState};

#[cfg(target_os = "windows")]
fn lease() -> InteractionLease {
    InteractionLease::guarded(Deadline::after(10_000).unwrap(), ()).expect("lease")
}

#[cfg(target_os = "windows")]
fn destroyed_window(process_instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "gone".into(),
        app: "none.exe".into(),
        pid: ProcessId::from(1u32),
        process_instance: process_instance.map(str::to_string),
        bounds: None,
        state: WindowState::default(),
    }
}

#[cfg(target_os = "windows")]
#[test]
fn focus_window_refuses_a_destroyed_handle_before_any_window_write() {
    let win = destroyed_window(Some("windows-proc-v1:0:0"));
    let err = focus_window(&win, &lease()).unwrap_err();
    assert_eq!(err.code, ErrorCode::WindowNotFound);
}

#[cfg(target_os = "windows")]
#[test]
fn focus_window_requires_a_process_instance_token() {
    let win = destroyed_window(None);
    let err = focus_window(&win, &lease()).unwrap_err();
    assert_eq!(err.code, ErrorCode::StaleRef);
}

/// Windows refuses `SetForegroundWindow` from a process that does not own
/// the foreground (A21-6), which is every headed action on a window in the
/// background. The raise must stay on the UI Automation focus route (A31-9).
#[test]
fn the_raise_asks_ui_automation_for_focus_and_never_calls_set_foreground_window() {
    let src = include_str!("window_activate.rs");
    let raise = src
        .split("fn bring_to_foreground")
        .nth(1)
        .expect("the raise function exists");
    assert!(raise.contains("focus_window_element(handle)"));
    assert!(!src.contains("SetForegroundWindow(handle)"));
}
