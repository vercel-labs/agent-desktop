use super::{capture_display, interop_is_available, modern_is_supported};
use crate::system::display::{display_at, list_displays_live};
use crate::system::permissions::report;
use crate::system::test_support::apartment::bootstrap;
use agent_desktop_core::PermissionState;
use windows::Graphics::Capture::GraphicsCaptureSession;

use crate::system::test_support::test_time::deadline;

use crate::system::capture::capture_test_support::live_graphics_capture_lock;

/// The monitor capture leg needs the HWND/HMONITOR interop, which is a
/// stricter gate than [`modern_is_supported`] (A22-1: IsSupported can be true
/// while `IGraphicsCaptureItemInterop` QI still fails).
fn skip_if_interop_unavailable(reason: &str) -> bool {
    if !modern_is_supported() {
        eprintln!("skip: {reason} (GraphicsCaptureSession::IsSupported is false)");
        return true;
    }
    if !interop_is_available() {
        eprintln!(
            "skip: {reason} (IsSupported true but IGraphicsCaptureItemInterop unavailable — A22-1)"
        );
        return true;
    }
    false
}

#[test]
fn support_predicate_is_the_capture_api_answer_and_stable_within_a_process() {
    bootstrap();
    let direct = GraphicsCaptureSession::IsSupported().unwrap_or(false);
    assert_eq!(
        modern_is_supported(),
        direct,
        "the predicate must be the capture API's own answer, not a build-number guess"
    );
    assert_eq!(
        modern_is_supported(),
        direct,
        "IsSupported must be stable for the process"
    );
}

#[test]
fn monitor_capture_returns_monitor_dimensions_when_supported() {
    let _capture = live_graphics_capture_lock();
    bootstrap();
    if skip_if_interop_unavailable("monitor capture") {
        return;
    }
    let primary = display_at(0, deadline(10_000)).expect("primary display");
    let image = capture_display(0, deadline(10_000)).expect("WGC monitor capture succeeds");
    assert_eq!(
        (image.width, image.height),
        (primary.bounds.width as u32, primary.bounds.height as u32)
    );
    assert_eq!(image.scale_factor, primary.scale);
}

#[test]
fn permission_report_screen_recording_is_honest_when_legacy_can_capture() {
    bootstrap();
    let displays = list_displays_live(deadline(10_000)).expect("displays");
    assert!(!displays.is_empty());
    let report = report(deadline(10_000)).expect("permission report");
    assert_eq!(
        report.screen_recording,
        PermissionState::NotRequired,
        "a session that can list displays can capture via Legacy"
    );
}
