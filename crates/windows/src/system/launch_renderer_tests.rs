//! Renderer detection from a window id that cannot name a real window.

use agent_desktop_core::ProcessId;

use crate::system::launch::detect_renderer_from_window;

fn window_info_for_id(id: &str) -> agent_desktop_core::WindowInfo {
    agent_desktop_core::WindowInfo {
        id: id.to_string(),
        title: String::new(),
        app: String::new(),
        pid: ProcessId::from(std::process::id()),
        process_instance: None,
        bounds: None,
        state: agent_desktop_core::WindowState::default(),
    }
}

/// The null-handle guard: an id `parse_handle` cannot resolve to a real
/// `HWND` must never reach `window_class_name`, whatever shape the id takes -
/// a string with no `w-` prefix at all, and a `w-` prefix whose number parses
/// but is the null handle itself.
#[cfg(target_os = "windows")]
#[test]
fn an_id_that_does_not_resolve_to_a_real_handle_reports_no_renderer() {
    assert!(detect_renderer_from_window(&window_info_for_id("not-a-window-id")).is_none());
    assert!(detect_renderer_from_window(&window_info_for_id("w-0")).is_none());
}
