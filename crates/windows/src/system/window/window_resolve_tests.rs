use super::*;
use agent_desktop_core::WindowState;

/// A `WindowInfo` naming a handle no live window answers for, so every test
/// below drives the same destroyed-handle shape and differs only in the check
/// it is making. `process_instance` is the parameter because the token's
/// absence is itself one of those checks.
#[cfg(target_os = "windows")]
fn destroyed_window(process_instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "gone".into(),
        app: "none.exe".into(),
        pid: agent_desktop_core::ProcessId::from(1u32),
        process_instance: process_instance.map(str::to_string),
        bounds: None,
        state: WindowState::default(),
    }
}

#[cfg(target_os = "windows")]
#[test]
fn resolve_window_strict_rejects_a_destroyed_handle() {
    let win = destroyed_window(Some("windows-proc-v1:0:0"));
    let err = resolve_window_strict(&win, Deadline::after(1_000).unwrap()).unwrap_err();
    assert_eq!(err.code, ErrorCode::WindowNotFound);
}

#[cfg(target_os = "windows")]
#[test]
fn resolve_window_strict_refuses_an_expired_deadline() {
    let win = destroyed_window(Some("windows-proc-v1:0:0"));
    let deadline = Deadline::after(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let err = resolve_window_strict(&win, deadline).unwrap_err();
    assert_eq!(err.code, ErrorCode::Timeout);
}
