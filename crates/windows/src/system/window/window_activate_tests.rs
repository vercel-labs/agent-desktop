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
