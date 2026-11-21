use std::cell::Cell;

use agent_desktop_core::{AdapterError, ErrorCode, ProcessId, SurfaceOpening, WindowInfo};

use super::opening_of;

fn window() -> WindowInfo {
    WindowInfo {
        id: "w-4242".into(),
        title: "Action center".into(),
        app: "ShellExperienceHost.exe".into(),
        pid: ProcessId::new(9),
        process_instance: None,
        bounds: None,
        state: Default::default(),
    }
}

#[test]
fn a_surface_found_presented_is_present_and_is_never_raised() {
    let raised = Cell::new(false);

    let opening = opening_of(Some(window()), || {
        raised.set(true);
        Ok(window())
    })
    .expect("a presented surface opens");

    assert!(matches!(opening, SurfaceOpening::Present(_)));
    assert!(!raised.get());
}

#[test]
fn a_surface_that_had_to_be_raised_is_raised_with_the_observed_window() {
    let opening = opening_of(None, || Ok(window())).expect("the raise is observed");

    assert!(matches!(opening, SurfaceOpening::Raised(ref raised) if raised.id == "w-4242"));
}

#[test]
fn a_raise_that_is_never_observed_is_an_error_not_an_opening() {
    let error = opening_of(None, || {
        Err(AdapterError::new(ErrorCode::Timeout, "did not open"))
    })
    .expect_err("an unobserved raise is not an open");

    assert_eq!(error.code, ErrorCode::Timeout);
}
