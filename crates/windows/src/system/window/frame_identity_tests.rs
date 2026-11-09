//! Windows-only: drives the frame-identity seam against the live desktop at
//! the listing level - the single-pass cost shape.
#![cfg(target_os = "windows")]

use crate::system::test_support::apartment::bootstrap;
use crate::system::test_support::test_time::deadline;
use crate::system::window::window_ops::{enumeration_calls, list_windows_live};
use agent_desktop_core::WindowFilter;

/// The rewrite's reads - the class read included - ride the single
/// enumeration pass `list_windows` already runs, with no second walk. The
/// no-match filter lists nothing, so no verification retry can inflate the
/// count: one listing call, exactly one enumeration.
#[test]
fn the_hosted_frame_detection_rides_the_existing_enumeration_pass() {
    bootstrap();
    enumeration_calls::take();
    let filter = WindowFilter {
        focused_only: false,
        app: Some(String::from("zz-agent-desktop-no-such-app")),
    };
    let windows = list_windows_live(&filter, deadline(15_000)).expect("the listing succeeds");
    assert!(windows.is_empty(), "the no-match filter lists nothing");
    assert_eq!(
        enumeration_calls::take(),
        1,
        "the identity reads ride the single enumeration pass"
    );
}
