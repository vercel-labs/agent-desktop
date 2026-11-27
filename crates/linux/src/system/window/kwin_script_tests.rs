use agent_desktop_core::{ProcessId, WindowInfo, WindowOp, WindowState};

use super::{close_pid_script, focus_script, op_script};

fn sample_window(pid: u32, title: &str) -> WindowInfo {
    WindowInfo {
        id: "w-1".to_owned(),
        title: title.to_owned(),
        app: "konsole".to_owned(),
        pid: ProcessId::new(pid),
        process_instance: Some("inst:1".to_owned()),
        bounds: None,
        state: WindowState::default(),
    }
}

#[test]
fn focus_script_escapes_title_and_unminimizes() {
    let js = focus_script(42, "a \"quoted\" title");
    assert!(js.contains("w.pid === 42"));
    assert!(js.contains(r#"w.caption === "a \"quoted\" title""#));
    assert!(js.contains("target.minimized = false;"));
    assert!(js.contains("workspace.activeWindow = target;"));
}

#[test]
fn op_script_generates_expected_kwin_mutations() {
    let win = sample_window(99, "Konsole");
    assert!(op_script(&win, &WindowOp::Move { x: 10.0, y: 20.0 }).contains("x: 10, y: 20"));
    assert!(
        op_script(
            &win,
            &WindowOp::Resize {
                width: 800.0,
                height: 600.0
            }
        )
        .contains("width: 800, height: 600")
    );
    assert!(op_script(&win, &WindowOp::Minimize).contains("target.minimized = true;"));
    assert!(op_script(&win, &WindowOp::Maximize).contains("target.setMaximize(true, true);"));
    assert!(op_script(&win, &WindowOp::Restore).contains("target.setMaximize(false, false);"));
}

#[test]
fn close_pid_script_targets_non_special_windows() {
    let js = close_pid_script(1234);
    assert!(js.contains("w.pid === 1234"));
    assert!(js.contains("w.closeWindow()"));
}
