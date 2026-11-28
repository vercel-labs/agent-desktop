use super::{AXElement, properties::element::element_for_pid, surface_read};
use agent_desktop_core::{AdapterError, Deadline, ErrorCode, SnapshotSurface, WindowInfo};

use crate::system::process::process_identity::ProcessIdentity;

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

/// The menu or popover the application shows, read in one bounded pass.
#[cfg(target_os = "macos")]
pub(crate) fn transient_surface_for_process(
    identity: ProcessIdentity,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    super::app_menu::observe(identity, deadline, || {
        super::app_menu::open_transient_surface(identity.pid(), deadline)
    })
}

/// The menu or popover inside `window`, searched at any depth because a popover
/// hangs below the control that raised it. The window is resolved inside the
/// observed pass, so a window that is gone and a process that was replaced both
/// fail instead of reading as an absent surface.
#[cfg(target_os = "macos")]
pub(crate) fn transient_surface_in_window(
    identity: ProcessIdentity,
    window: &WindowInfo,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    super::app_menu::observe(identity, deadline, || {
        let owner =
            crate::system::window::window_resolve::window_element_for_info(window, deadline)?;
        Ok(
            super::app_menu::find_transient_surface(owner, "window".into(), deadline)?
                .map(|(_, surface)| surface),
        )
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

/// The first popover in `window`, nearest the window first, through a search
/// that opens every element below it, so a popover anchored to a control inside
/// a list or a table row is reached. None only when the search read every
/// element: one cut at its element limit fails as not found and says it is
/// incomplete.
#[cfg(target_os = "macos")]
pub(super) fn popover_in_window(
    window: &AXElement,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let found = super::app_menu::find_surface_descendant_with(
        window.clone(),
        "window".into(),
        super::app_menu::ChildExpansion::EveryNode,
        deadline,
        |element, _| {
            if role_or_subrole_matches(element, "AXPopover", deadline)? {
                return Ok((true, Vec::new()));
            }
            Ok((
                false,
                surface_read::elements(element, "AXChildren", deadline)?,
            ))
        },
    )
    .map_err(popover_search_error)?;
    Ok(found.map(|(_, popover)| popover))
}

fn popover_search_error(error: AdapterError) -> AdapterError {
    if !super::app_menu::is_surface_search_limit(&error) {
        return error;
    }
    popover_search_cut()
}

/// A popover search that did not read every element: it reached its element
/// limit first, and a popover may sit below that cut.
fn popover_search_cut() -> AdapterError {
    AdapterError::new(
        ErrorCode::ElementNotFound,
        "No popover among the elements the search read; the search reached its element limit first",
    )
    .with_details(serde_json::json!({
        "kind": "popover_search_limit",
        "complete": false,
    }))
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
            Some("AXDialog" | "AXSystemDialog" | "AXAlert" | "AXSheet")
        ))
}

#[cfg(target_os = "macos")]
pub(crate) fn is_menu_open(pid: i32, deadline: Deadline) -> Result<bool, AdapterError> {
    Ok(menu_element_for_pid(pid, deadline)?.is_some())
}
