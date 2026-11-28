use super::NativeWindow;
use agent_desktop_core::{DeliverySemantics, ErrorCode, ProcessId};

fn own_instance() -> String {
    crate::system::process_identity::token_for_pid(ProcessId::new(std::process::id()))
        .expect("own token")
        .expect("own token present")
}

#[test]
fn a_window_another_process_owns_is_never_messaged_for_the_target() {
    let desktop =
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetDesktopWindow() } as isize;
    let target = NativeWindow {
        hwnd: desktop,
        pid: std::process::id(),
        instance: own_instance(),
    };

    let error = target.ensure_owned(desktop).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(target.ensure_owned(0).is_err());
}
