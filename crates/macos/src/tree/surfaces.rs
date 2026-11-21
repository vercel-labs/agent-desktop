use super::{AXElement, properties::element::element_for_pid, surface_read};
use agent_desktop_core::{AdapterError, Deadline, ErrorCode, SnapshotSurface};

#[cfg(target_os = "macos")]
const POPOVER_SEARCH_LIMIT: usize = 4_096;

#[cfg(target_os = "macos")]
pub(crate) fn focused_surface_for_pid(
    pid: i32,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    surface_read::element(&app, "AXFocusedWindow", deadline)
}

#[cfg(target_os = "macos")]
pub(crate) fn menu_element_for_pid(
    pid: i32,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)?
        .ok_or_else(|| AdapterError::stale_ref_because("Menu application exited"))?;
    super::app_menu::observe(identity, deadline, || {
        super::app_menu::root_for_pid(pid, SnapshotSurface::Menu, deadline)
    })
}

#[cfg(target_os = "macos")]
fn first_child_with_role_or_subrole(
    pid: i32,
    target: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(window) = focused_surface_for_pid(pid, deadline)? else {
        return Ok(None);
    };
    first_in_window(&window, deadline, |element| {
        role_or_subrole_matches(element, target, deadline)
    })
}

/// Finds a sheet, popover or alert on this window. A sheet or an alert is the
/// window or one of its direct children. A popover hangs below the control it
/// is anchored to, at any depth, so it is searched for through the window.
#[cfg(target_os = "macos")]
pub(crate) fn surface_in_window(
    window: &AXElement,
    surface: SnapshotSurface,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    match surface {
        SnapshotSurface::Sheet => first_in_window(window, deadline, |element| {
            role_or_subrole_matches(element, "AXSheet", deadline)
        }),
        SnapshotSurface::Popover => popover_in_window(window, deadline),
        SnapshotSurface::Alert => {
            first_in_window(window, deadline, |element| is_alert(element, deadline))
        }
        _ => Err(AdapterError::not_supported("window-owned surface")),
    }
}

/// The first popover in `window`, nearest the window first. None only when
/// every element was read: a search cut at `POPOVER_SEARCH_LIMIT` elements
/// fails as not found and says it is incomplete.
#[cfg(target_os = "macos")]
fn popover_in_window(
    window: &AXElement,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let mut level = vec![window.clone()];
    let mut searched = 0;
    while !level.is_empty() {
        let mut below = Vec::new();
        for element in level {
            if role_or_subrole_matches(&element, "AXPopover", deadline)? {
                return Ok(Some(element));
            }
            searched += 1;
            if searched >= POPOVER_SEARCH_LIMIT {
                return Err(AdapterError::new(
                    ErrorCode::ElementNotFound,
                    format!("No popover among the first {searched} elements of the window; the rest was not searched"),
                )
                .with_details(serde_json::json!({
                    "kind": "popover_search_limit",
                    "complete": false,
                })));
            }
            below.extend(surface_read::elements(&element, "AXChildren", deadline)?);
        }
        level = below;
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn first_in_window(
    window: &AXElement,
    deadline: Deadline,
    matches: impl Fn(&AXElement) -> Result<bool, AdapterError>,
) -> Result<Option<AXElement>, AdapterError> {
    if matches(window)? {
        return Ok(Some(window.clone()));
    }
    for child in surface_read::elements(window, "AXChildren", deadline)? {
        if matches(&child)? {
            return Ok(Some(child));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
pub(super) fn role_or_subrole_matches(
    element: &AXElement,
    target: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if surface_read::string(element, "AXRole", deadline)?.as_deref() == Some(target) {
        return Ok(true);
    }
    Ok(surface_read::string(element, "AXSubrole", deadline)?.as_deref() == Some(target))
}

#[cfg(target_os = "macos")]
pub(crate) fn sheet_for_pid(
    pid: i32,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    first_child_with_role_or_subrole(pid, "AXSheet", deadline)
}

#[cfg(target_os = "macos")]
pub(crate) fn popover_for_pid(
    pid: i32,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    match focused_surface_for_pid(pid, deadline)? {
        Some(window) => popover_in_window(&window, deadline),
        None => Ok(None),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn alert_for_pid(
    pid: i32,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    let mut windows = surface_read::elements(&app, "AXWindows", deadline)?;
    if let Some(focused) = focused_surface_for_pid(pid, deadline)? {
        windows.insert(0, focused);
    }
    for window in windows {
        if is_alert(&window, deadline)? {
            return Ok(Some(window));
        }
        for child in surface_read::elements(&window, "AXChildren", deadline)? {
            if is_alert(&child, deadline)? {
                return Ok(Some(child));
            }
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
pub(super) fn is_alert(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    let role = surface_read::string(element, "AXRole", deadline)?;
    let subrole = surface_read::string(element, "AXSubrole", deadline)?;
    Ok(matches!(role.as_deref(), Some("AXSheet"))
        || matches!(
            subrole.as_deref(),
            Some("AXDialog") | Some("AXAlert") | Some("AXSheet")
        ))
}

#[cfg(target_os = "macos")]
pub(crate) fn is_menu_open(pid: i32, deadline: Deadline) -> Result<bool, AdapterError> {
    Ok(menu_element_for_pid(pid, deadline)?.is_some())
}
