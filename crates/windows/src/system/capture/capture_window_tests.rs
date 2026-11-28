use agent_desktop_core::ErrorCode;

#[test]
fn printwindow_access_denied_maps_to_perm_denied() {
    unsafe { windows_sys::Win32::Foundation::SetLastError(5) };
    let error = super::win32_last_error("PrintWindow failed");
    assert_eq!(error.code, ErrorCode::PermDenied);
    assert!(
        error
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("0x80070005"))
    );
}

#[test]
fn oversized_dimensions_are_rejected_while_ordinary_dimensions_are_accepted() {
    let oversized = crate::system::capture::gdi_surface::reject_oversized_capture(23171, 23171)
        .expect_err("a window whose pixel byte count overflows i32 must be refused");
    assert_eq!(oversized.code, ErrorCode::InvalidArgs);

    crate::system::capture::gdi_surface::reject_oversized_capture(1920, 1080)
        .expect("an ordinary window size must still be accepted");
}
