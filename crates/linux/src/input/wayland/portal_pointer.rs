use std::thread::sleep;
use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, DragParams, ErrorCode, MouseButton, MouseEvent, MouseEventKind,
    OperationReport, validate_mouse_click_count,
};

use super::key_session::KeySession;
use super::keysym_send::pointer_refusal;
use super::portal_remote_desktop::PortalRemoteDesktop;
use crate::input::delivery::Delivery;
use crate::input::mouse::pointer_report::with_event_count;

const BTN_LEFT: u32 = 272;
const BTN_RIGHT: u32 = 273;
const BTN_MIDDLE: u32 = 274;
const AXIS_VERTICAL: u32 = 0;
const AXIS_HORIZONTAL: u32 = 1;
const SETTLE: Duration = Duration::from_millis(60);
const CLICK_HOLD: Duration = Duration::from_millis(25);
const CLICK_GAP: Duration = Duration::from_millis(40);
const DRAG_STEPS: u32 = 16;
const RECOVERY_MS: u64 = 1_000;

fn evdev_button(button: &MouseButton) -> u32 {
    match button {
        MouseButton::Left => BTN_LEFT,
        MouseButton::Right => BTN_RIGHT,
        MouseButton::Middle => BTN_MIDDLE,
    }
}

fn recovery(spent: Deadline) -> Deadline {
    Deadline::detached_after(RECOVERY_MS).unwrap_or(spent)
}

fn open_portal(
    what: &'static str,
    deadline: Deadline,
) -> Result<PortalRemoteDesktop, AdapterError> {
    let mut portal = match PortalRemoteDesktop::open(deadline, true) {
        Ok(portal) => portal,
        Err(error) if error.code == ErrorCode::ActionNotSupported => {
            return Err(pointer_refusal(what));
        }
        Err(error) => return Err(error),
    };
    portal.create(deadline)?;
    if let Err(error) = portal.start(deadline) {
        let _ = portal.stop(recovery(deadline));
        return Err(error);
    }
    Ok(portal)
}

pub(crate) fn send_mouse(
    event: &MouseEvent,
    deadline: Deadline,
) -> Result<OperationReport, AdapterError> {
    event.point.validate()?;
    if let MouseEventKind::Click { count } = event.kind {
        validate_mouse_click_count(count)?;
    }
    let mut portal = open_portal("mouse_event", deadline)?;
    let result = dispatch_mouse(&mut portal, event, deadline);
    let _ = portal.stop(recovery(deadline));
    let delivered_events = result?;
    Ok(with_event_count(
        OperationReport::pointer_dispatched(event, false),
        &Delivery { delivered_events },
    ))
}

fn dispatch_mouse(
    portal: &mut PortalRemoteDesktop,
    event: &MouseEvent,
    deadline: Deadline,
) -> Result<u32, AdapterError> {
    let mut count = 0_u32;
    if portal.stream_node().is_some() {
        portal.pointer_motion_absolute(event.point.x, event.point.y, deadline)?;
        count += 1;
        sleep(SETTLE.min(deadline.remaining()));
    }
    match &event.kind {
        MouseEventKind::Move => Ok(count),
        MouseEventKind::Click { count: clicks } => {
            let code = evdev_button(&event.button);
            for index in 0..*clicks {
                portal.pointer_button(code, true, deadline)?;
                count += 1;
                sleep(CLICK_HOLD.min(deadline.remaining()));
                portal.pointer_button(code, false, recovery(deadline))?;
                count += 1;
                if index + 1 < *clicks {
                    sleep(CLICK_GAP.min(deadline.remaining()));
                }
            }
            Ok(count)
        }
        MouseEventKind::Wheel { delta_x, delta_y } => {
            let steps_y = delta_y.round() as i32;
            if steps_y != 0 {
                portal.pointer_axis_discrete(AXIS_VERTICAL, -steps_y, deadline)?;
                count += 1;
            }
            let steps_x = delta_x.round() as i32;
            if steps_x != 0 {
                portal.pointer_axis_discrete(AXIS_HORIZONTAL, steps_x, deadline)?;
                count += 1;
            }
            Ok(count)
        }
        MouseEventKind::Down | MouseEventKind::Up => Err(pointer_refusal("mouse_event")),
    }
}

pub(crate) fn send_drag(
    params: &DragParams,
    deadline: Deadline,
) -> Result<OperationReport, AdapterError> {
    params.from.validate()?;
    params.to.validate()?;
    let mut portal = open_portal("drag", deadline)?;
    let result = dispatch_drag(&mut portal, params, deadline);
    let _ = portal.stop(recovery(deadline));
    let delivered_events = result?;
    Ok(with_event_count(
        OperationReport::drag_dispatched(params),
        &Delivery { delivered_events },
    ))
}

fn dispatch_drag(
    portal: &mut PortalRemoteDesktop,
    params: &DragParams,
    deadline: Deadline,
) -> Result<u32, AdapterError> {
    let mut count = 0_u32;
    portal.pointer_motion_absolute(params.from.x, params.from.y, deadline)?;
    count += 1;
    sleep(SETTLE.min(deadline.remaining()));
    portal.pointer_button(BTN_LEFT, true, deadline)?;
    count += 1;
    let duration_ms = params.duration_ms.unwrap_or(300).max(100);
    let step_delay = Duration::from_millis(duration_ms / u64::from(DRAG_STEPS));
    for step in 1..=DRAG_STEPS {
        let progress = f64::from(step) / f64::from(DRAG_STEPS);
        let x = params.from.x + (params.to.x - params.from.x) * progress;
        let y = params.from.y + (params.to.y - params.from.y) * progress;
        if let Err(error) = portal.pointer_motion_absolute(x, y, deadline) {
            let _ = portal.pointer_button(BTN_LEFT, false, recovery(deadline));
            return Err(error);
        }
        count += 1;
        sleep(step_delay.min(deadline.remaining()));
    }
    let drop_delay = Duration::from_millis(params.drop_delay_ms.unwrap_or(150));
    sleep(drop_delay.min(deadline.remaining()));
    portal.pointer_button(BTN_LEFT, false, recovery(deadline))?;
    count += 1;
    Ok(count)
}
