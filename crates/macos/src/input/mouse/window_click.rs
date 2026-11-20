use super::window_click_route::WindowClickRoute;
use crate::actions::DeliveryTracker;
use crate::system::window::{
    cg_window::{WindowRecordScope, window_records_until},
    cg_window_exact::exact_window_record_until,
    window_resolve::{parse_window_number, resolve_capture_window},
};
use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, ErrorCode, MouseEvent, OperationReport,
    OperationTarget, Point, Rect, StepMechanism, WindowInfo,
};

pub(crate) fn click_window(
    window_id: &str,
    event: MouseEvent,
    deadline: Deadline,
) -> Result<OperationReport, AdapterError> {
    dispatch_window_click(window_id, event, deadline)
        .map_err(|error| DeliveryTracker::default().annotate(error))
}

fn dispatch_window_click(
    window_id: &str,
    mut event: MouseEvent,
    deadline: Deadline,
) -> Result<OperationReport, AdapterError> {
    if !unsafe { CGPreflightPostEventAccess() } {
        return Err(AdapterError::new(
            ErrorCode::PermDenied,
            "macOS event posting permission is not granted",
        )
        .with_suggestion("Grant Accessibility permission to the app running agent-desktop"));
    }
    let number = parse_window_number(window_id)
        .filter(|number| u32::try_from(*number).is_ok())
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::InvalidArgs,
                "Use a window ID from 'list-windows'",
            )
        })?;
    let until = crate::tree::locator_deadline::from_operation(deadline)?;
    let record = exact_window_record_until(number, until)?.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::WindowNotFound,
            format!("Window '{window_id}' was not found"),
        )
        .with_suggestion("Run 'list-windows' to refresh window IDs, then retry")
    })?;
    let pid = record.pid;
    let window = record.into_window_info(false, None, false)?;
    let bounds = visible_bounds(&window)?;
    let route = WindowClickRoute::new(pid, number, &event.point)?;
    event.point = screen_point(bounds, &event.point)?;
    let target = OperationTarget::for_window(&window)?;
    let mut verify_target = || {
        let live = resolve_capture_window(&window, until)?;
        if visible_bounds(&live)? != bounds {
            return Err(AdapterError::new(
                ErrorCode::StaleRef,
                "Window frame changed before the click",
            )
            .with_suggestion("Capture the window again before retrying"));
        }
        let siblings = window_records_until(until, WindowRecordScope::Pid(pid))?;
        let first = siblings.iter().find(|record| record.visible);
        if first.is_none_or(|record| record.window_number != number) {
            return Err(AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Clicking a window behind another window of its app can change window order",
            )
            .with_suggestion("Use a ref action or choose the frontmost window of that app"));
        }
        if first.is_some_and(|record| {
            record.bounds != bounds || record.process_instance != window.process_instance
        }) {
            return Err(AdapterError::new(
                ErrorCode::StaleRef,
                "Window identity or frame changed before the click",
            )
            .with_suggestion("Capture the window again before retrying"));
        }
        Ok(())
    };
    super::imp::synthesize_window_click(event, deadline, &mut verify_target, &|event| {
        route.post(event);
    })?;
    let result = ActionResult::delivered_unverified("mouse-click").with_steps(vec![
        ActionStep::succeeded("CGEventPostToPid")
            .with_mechanism(StepMechanism::PhysicalSynthetic)
            .with_verified(false),
    ]);
    Ok(OperationReport::new(result, target))
}

fn visible_bounds(window: &WindowInfo) -> Result<Rect, AdapterError> {
    if window.state.visible != Some(true) {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Window-relative clicks require an on-screen window",
        ));
    }
    window
        .bounds
        .ok_or_else(|| AdapterError::new(ErrorCode::ActionNotSupported, "Window frame is unknown"))?
        .validate()
}

fn screen_point(bounds: Rect, local: &Point) -> Result<Point, AdapterError> {
    if !(0.0..bounds.width).contains(&local.x) || !(0.0..bounds.height).contains(&local.y) {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "--window-xy must be inside the window frame",
        ));
    }
    Ok(Point {
        x: bounds.x + local.x,
        y: bounds.y + local.y,
    })
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightPostEventAccess() -> bool;
}
