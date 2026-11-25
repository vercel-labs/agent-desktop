//! What the front guard reads and does on macOS: the frontmost application, the
//! window order from CoreGraphics (front to back), and a re-activation that
//! raises the user's windows over an intruder.

use agent_desktop_core::{AdapterError, Deadline, FrontView, FrontWindow};
use core_foundation::string::CFString;

use super::cg_window::{WindowDictionary, bool_field, required_int_field, window_dictionaries};
use crate::system::app::workspace_apps::focused_owner_until;
use crate::system::focus_bridge;
use crate::system::process::process_identity::ProcessIdentity;

/// One window of the screen's order: its id, its owner and whether it is on
/// screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrderedWindow {
    pub(crate) number: i64,
    pub(crate) pid: i32,
    pub(crate) on_screen: bool,
}

pub(crate) fn read_front(deadline: Deadline) -> Result<FrontView, AdapterError> {
    let frontmost = focused_owner_until(deadline)?.map(|owner| owner.pid);
    let order = ordered_windows(window_dictionaries()?)?;
    Ok(view_of(&order, frontmost))
}

/// The front view of a window order that runs front to back. The focused
/// window is the frontmost application's topmost window on screen.
pub(crate) fn view_of(order: &[OrderedWindow], frontmost: Option<i32>) -> FrontView {
    let window_of = |window: &OrderedWindow| FrontWindow {
        id: window_id(window.number),
        pid: u32::try_from(window.pid).unwrap_or_default(),
    };
    let mut on_screen = order.iter().filter(|window| window.on_screen);
    FrontView {
        focused: frontmost
            .and_then(|pid| {
                order
                    .iter()
                    .find(|window| window.on_screen && window.pid == pid)
            })
            .map(window_of),
        top: on_screen.next().map(window_of),
        windows: order
            .iter()
            .map(|window| window_id(window.number))
            .collect(),
    }
}

fn window_id(number: i64) -> String {
    format!("w-{number}")
}

fn ordered_windows(
    dictionaries: Vec<WindowDictionary>,
) -> Result<Vec<OrderedWindow>, AdapterError> {
    let mut order = Vec::new();
    for dictionary in dictionaries {
        if required_int_field(&dictionary, "kCGWindowLayer")? != 0 {
            continue;
        }
        let pid = i32::try_from(required_int_field(&dictionary, "kCGWindowOwnerPID")?)
            .unwrap_or_default();
        if pid <= 0
            || dictionary
                .find(CFString::new("kCGWindowOwnerName"))
                .is_none()
        {
            continue;
        }
        order.push(OrderedWindow {
            number: required_int_field(&dictionary, "kCGWindowNumber")?,
            pid,
            on_screen: bool_field(&dictionary, "kCGWindowIsOnscreen")?,
        });
    }
    Ok(order)
}

/// Re-activating the user's application raises its windows over the
/// intruder's. macOS has no call that places one window below another.
pub(crate) fn give_back(window: &FrontWindow) -> Result<(), AdapterError> {
    let pid = i32::try_from(window.pid)
        .map_err(|_| AdapterError::internal("front window pid is out of range"))?;
    let identity = ProcessIdentity::capture(pid)?
        .ok_or_else(|| AdapterError::stale_ref_because("The front application has exited"))?;
    focus_bridge::activate(identity).map(drop)
}

#[cfg(test)]
#[path = "window_front_tests.rs"]
mod tests;
