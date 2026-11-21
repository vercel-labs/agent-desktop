//! Surface-root resolution tests: the per-surface match arms.

use super::*;

#[cfg(target_os = "windows")]
#[test]
fn a_malformed_window_id_is_rejected_before_the_platform_is_reached() {
    let error = parse_window_id("not-a-window").expect_err("must reject");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

/// `window_is_modal_sheet` reads the property with no provider; an absent
/// or unknown read is not a sheet.
#[cfg(not(target_os = "windows"))]
#[test]
fn an_absent_window_modal_read_is_not_a_sheet() {
    use super::super::element::{CannedElement, UIAElement};
    let element = UIAElement::from(CannedElement);
    assert!(!window_is_modal_sheet(&element));
}
