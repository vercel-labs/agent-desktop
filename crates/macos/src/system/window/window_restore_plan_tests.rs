use super::*;
use agent_desktop_core::ProcessIdentity;

fn frame(x: f64, width: f64) -> Rect {
    Rect {
        x,
        y: 30.0,
        width,
        height: 500.0,
    }
}

fn record() -> WindowFrameRecord {
    WindowFrameRecord {
        process: ProcessIdentity::new(7u32, "boot"),
        before: frame(100.0, 400.0),
        maximized: frame(0.0, 1496.0),
    }
}

fn no_work_area(_: Rect) -> Result<Rect, AdapterError> {
    panic!("work area must not be read")
}

fn fills(current: Rect) -> Result<Rect, AdapterError> {
    Ok(current)
}

fn elsewhere_work_area(_: Rect) -> Result<Rect, AdapterError> {
    Ok(frame(0.0, 1496.0))
}

#[test]
fn a_minimized_window_is_unminimized_whatever_was_recorded() {
    let plan = WindowRestorePlan::decide(
        Some(true),
        Some(&record()),
        || panic!("frame must not be read"),
        no_work_area,
    );
    assert_eq!(plan.ok(), Some(WindowRestorePlan::Unminimize));
}

#[test]
fn a_window_still_at_its_maximized_frame_goes_back_to_the_recorded_frame() {
    let plan = WindowRestorePlan::decide(
        Some(false),
        Some(&record()),
        || Ok(frame(1.0, 1495.0)),
        no_work_area,
    );
    assert_eq!(
        plan.ok(),
        Some(WindowRestorePlan::SetFrame(frame(100.0, 400.0)))
    );
}

#[test]
fn a_half_applied_maximize_goes_back_to_the_recorded_frame() {
    let plan = WindowRestorePlan::decide(
        Some(false),
        Some(&record()),
        || Ok(frame(0.0, 400.0)),
        no_work_area,
    );
    assert_eq!(
        plan.ok(),
        Some(WindowRestorePlan::SetFrame(frame(100.0, 400.0)))
    );
}

#[test]
fn a_window_at_its_recorded_frame_is_already_normal() {
    let plan = WindowRestorePlan::decide(
        Some(false),
        Some(&record()),
        || Ok(frame(100.0, 400.0)),
        no_work_area,
    );
    assert_eq!(plan.ok(), Some(WindowRestorePlan::AlreadyNormal));
}

#[test]
fn a_maximized_window_without_a_record_is_unrecorded() {
    let plan = WindowRestorePlan::decide(Some(false), None, || Ok(frame(0.0, 1496.0)), fills);
    assert_eq!(plan.ok(), Some(WindowRestorePlan::Unrecorded));
}

#[test]
fn an_ordinary_window_without_a_record_is_already_normal() {
    let plan = WindowRestorePlan::decide(
        Some(false),
        None,
        || Ok(frame(300.0, 400.0)),
        elsewhere_work_area,
    );
    assert_eq!(plan.ok(), Some(WindowRestorePlan::AlreadyNormal));
}

#[test]
fn a_stale_record_on_a_maximized_window_is_unrecorded() {
    let plan = WindowRestorePlan::decide(
        Some(false),
        Some(&record()),
        || Ok(frame(300.0, 1496.0)),
        fills,
    );
    assert_eq!(plan.ok(), Some(WindowRestorePlan::Unrecorded));
}

#[test]
fn an_unreadable_frame_is_an_error_so_the_record_is_kept() {
    let plan = WindowRestorePlan::decide(
        Some(false),
        Some(&record()),
        || Err(AdapterError::internal("window frame is unreadable")),
        no_work_area,
    );
    assert!(plan.is_err());
}

#[test]
fn an_unreadable_work_area_is_an_error() {
    let plan = WindowRestorePlan::decide(
        Some(false),
        None,
        || Ok(frame(0.0, 1496.0)),
        |_| Err(AdapterError::internal("work area is unreadable")),
    );
    assert!(plan.is_err());
}

#[test]
fn headless_policy_refuses_to_unminimize_and_headed_allows_it() {
    let refused = WindowRestorePlan::Unminimize
        .permitted_by(InteractionPolicy::headless())
        .unwrap_err();
    assert_eq!(refused.code.as_str(), "POLICY_DENIED");
    assert!(refused.suggestion.unwrap().contains("--headed"));
    assert_eq!(
        WindowRestorePlan::Unminimize
            .permitted_by(InteractionPolicy::headed())
            .unwrap(),
        WindowRestorePlan::Unminimize
    );
}

#[test]
fn headless_policy_keeps_frame_moves_quiet() {
    for plan in [
        WindowRestorePlan::SetFrame(frame(0.0, 400.0)),
        WindowRestorePlan::AlreadyNormal,
        WindowRestorePlan::Unrecorded,
    ] {
        assert_eq!(
            plan.permitted_by(InteractionPolicy::headless()).unwrap(),
            plan
        );
    }
}
