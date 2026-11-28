use super::screenshot;
use crate::system::capture::capture_backend::test_hooks as backend_hooks;
use crate::system::display::display_at;
use crate::system::test_support::apartment::bootstrap;
use agent_desktop_core::{
    DeliveryDisposition, ErrorCode, ImageFormat, ScreenshotTarget, parse_png_dimensions,
};

use crate::system::test_support::test_time::deadline;

use crate::system::capture::capture_test_support::live_graphics_capture_lock;

fn assert_png_metadata(image: &agent_desktop_core::ImageBuffer) {
    assert_eq!(image.format.as_str(), ImageFormat::Png.as_str());
    let (width, height) =
        parse_png_dimensions(&image.data).expect("PNG header must parse for core consumers");
    assert_eq!((width, height), (image.width, image.height));
}

#[test]
fn modern_forced_fail_after_available_falls_back_to_legacy() {
    let _capture = live_graphics_capture_lock();
    bootstrap();
    backend_hooks::with_force_fail_after_available(|| {
        let image = screenshot(ScreenshotTarget::Screen(0), deadline(10_000))
            .expect("modern failure must fall back to legacy");
        assert_png_metadata(&image);
    });
}

#[test]
fn display_identity_mismatch_is_not_delivered_through_the_entry_point() {
    let _capture = live_graphics_capture_lock();
    bootstrap();
    let primary = display_at(0, deadline(10_000)).expect("primary");
    let mut stale = primary.clone();
    stale.bounds.width += 1.0;

    let error = screenshot(
        ScreenshotTarget::Display {
            index: 0,
            expected: stale,
        },
        deadline(10_000),
    )
    .expect_err("a stale display fingerprint must fail before native capture");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn screenshot_entry_upgrades_only_unknown_disposition_to_not_delivered() {
    use agent_desktop_core::{AdapterError, DeliverySemantics};

    let unknown = AdapterError::new(ErrorCode::InvalidArgs, "probe");
    let upgraded = super::coerce_screenshot_disposition_for_test(unknown);
    assert_eq!(upgraded.disposition, DeliverySemantics::not_delivered());

    let already = AdapterError::new(ErrorCode::InvalidArgs, "probe")
        .with_disposition(DeliverySemantics::not_delivered());
    assert_eq!(
        super::coerce_screenshot_disposition_for_test(already).disposition,
        DeliverySemantics::not_delivered()
    );

    for semantics in [
        DeliverySemantics::uncertain(),
        DeliverySemantics::delivered_unverified(),
        DeliverySemantics::delivered_verified(),
    ] {
        let error = AdapterError::new(ErrorCode::ActionFailed, "probe").with_disposition(semantics);
        assert_eq!(
            super::coerce_screenshot_disposition_for_test(error).disposition,
            semantics
        );
    }
}
#[test]
fn invalid_display_index_is_not_delivered_through_the_entry_point() {
    let _capture = live_graphics_capture_lock();
    bootstrap();
    let error = screenshot(ScreenshotTarget::Screen(9_999), deadline(10_000))
        .expect_err("an out-of-range display index must fail");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn adapter_screenshot_delegates_to_orchestration() {
    let _capture = live_graphics_capture_lock();
    use agent_desktop_core::SystemOps;

    bootstrap();
    let adapter = crate::adapter::WindowsAdapter::new();
    let image = SystemOps::screenshot(&adapter, ScreenshotTarget::FullScreen, deadline(10_000))
        .expect("SystemOps::screenshot must be wired");
    assert_png_metadata(&image);
}
