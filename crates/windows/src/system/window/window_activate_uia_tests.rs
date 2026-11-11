use super::*;
use agent_desktop_core::ErrorCode;

#[test]
fn provider_activation_refuses_recycled_generation_before_provider_access() {
    crate::tree::fixture::ensure_test_apartment();
    let fixture = crate::tree::fixture::LocalFixture::create().expect("fixture");
    let handle = fixture.handle() as WindowHandle;
    let mut info =
        crate::system::fixture::live_identity::window_info_for_current_process(fixture.handle());
    info.process_instance = Some("windows-proc-v1:0:0".into());
    let evidence = WindowIdentityEvidence::from_info(handle, &info).expect("evidence");
    let error = activate(handle, &evidence, Deadline::after(1000).expect("deadline"))
        .expect_err("recycled handle must not receive a provider mutation");
    assert_eq!(error.code, ErrorCode::WindowNotFound);
}

#[test]
fn provider_activation_preserves_normal_and_maximized_window_geometry() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, IsZoomed, SW_MAXIMIZE, SetForegroundWindow, ShowWindow,
    };
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    crate::tree::fixture::ensure_test_apartment();
    let target = crate::tree::fixture::LocalFixture::create_at(100, 100).expect("target");
    let other = crate::tree::fixture::LocalFixture::create_at(600, 100).expect("other");
    let info =
        crate::system::fixture::live_identity::window_info_for_current_process(target.handle());
    let handle = target.handle() as WindowHandle;
    let evidence = WindowIdentityEvidence::from_info(handle, &info).expect("evidence");
    for maximized in [false, true] {
        if maximized {
            unsafe { ShowWindow(handle, SW_MAXIMIZE) };
        }
        let before = native_bounds(handle);
        unsafe { SetForegroundWindow(other.handle() as _) };
        assert_ne!(
            unsafe { GetForegroundWindow() },
            handle,
            "target must start in the background"
        );
        activate(handle, &evidence, Deadline::after(2000).expect("deadline")).expect("activate");
        assert_eq!(unsafe { GetForegroundWindow() }, handle);
        assert_eq!(unsafe { IsZoomed(handle) } != 0, maximized);
        assert_eq!(native_bounds(handle), before);
    }
}

#[test]
fn provider_timeout_shrinks_with_the_shared_deadline_and_refuses_expiry() {
    crate::tree::fixture::ensure_test_apartment();
    let client: IUIAutomation2 =
        unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_ALL) }.expect("bounded client");
    let deadline = Deadline::after(5000).expect("deadline");
    configure_timeout(&client, deadline).expect("initial budget");
    let first = unsafe { client.TransactionTimeout() }.expect("timeout");
    let shortened = deadline.capped(std::time::Duration::from_millis(100));
    configure_timeout(&client, shortened).expect("remaining budget");
    let second = unsafe { client.TransactionTimeout() }.expect("timeout");
    assert!(
        second < first,
        "requests must not each receive the initial budget"
    );
    assert_eq!(
        configure_timeout(&client, Deadline::after(0).expect("expired"))
            .unwrap_err()
            .code(),
        windows::core::HRESULT(UIA_E_TIMEOUT as i32)
    );
}

#[test]
fn expired_budget_never_activates_the_window() {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    crate::tree::fixture::ensure_test_apartment();
    let fixture = crate::tree::fixture::LocalFixture::create().expect("fixture");
    let handle = fixture.handle() as WindowHandle;
    let info =
        crate::system::fixture::live_identity::window_info_for_current_process(fixture.handle());
    let evidence = WindowIdentityEvidence::from_info(handle, &info).expect("evidence");
    let before = unsafe { GetForegroundWindow() };
    assert_ne!(before, handle, "target must start in the background");
    activate(handle, &evidence, Deadline::after(0).expect("expired")).expect("no-op");
    assert_eq!(unsafe { GetForegroundWindow() }, before);
}

fn native_bounds(handle: WindowHandle) -> [i32; 4] {
    let mut rect = windows_sys::Win32::Foundation::RECT::default();
    assert_ne!(
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(handle, &mut rect) },
        0
    );
    [rect.left, rect.top, rect.right, rect.bottom]
}
