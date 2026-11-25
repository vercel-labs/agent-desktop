use crate::tree::{AXElement, properties::element::element_for_pid};
use agent_desktop_core::{AdapterError, Deadline, SurfaceInfo};

/// The most children an element that anchors a surface has. An element with
/// more is a content container: a list, a table or a column of a browser.
pub(crate) const ANCHOR_CHILD_LIMIT: usize = 16;

pub(crate) fn list_surfaces_for_process(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Deadline,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    super::app_menu::observe(identity, deadline, || {
        list_surfaces_once(identity.pid(), deadline)
    })
}

fn list_surfaces_once(pid: i32, deadline: Deadline) -> Result<Vec<SurfaceInfo>, AdapterError> {
    let app = element_for_pid(pid);
    list_surfaces_with(
        deadline,
        || {
            let mut surfaces = Vec::new();
            if let Some(window) = read_element(&app, "AXFocusedWindow", deadline)? {
                collect_window_surfaces(&window, "focused-window", deadline, &mut surfaces)?;
            }
            Ok(surfaces)
        },
        || {
            let mut surfaces = Vec::new();
            for (kind, id, menu) in super::app_menu::open_menus_for_pid(pid, deadline)? {
                push_menu_surface(&menu, kind, id, deadline, &mut surfaces)?;
            }
            Ok(surfaces)
        },
    )
}

fn list_surfaces_with(
    deadline: Deadline,
    window_surfaces: impl FnOnce() -> Result<Vec<SurfaceInfo>, AdapterError>,
    menu_surfaces: impl FnOnce() -> Result<Vec<SurfaceInfo>, AdapterError>,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    ensure_before_deadline(deadline)?;
    let mut surfaces = menu_surfaces()?;
    surfaces.extend(window_surfaces()?);
    ensure_before_deadline(deadline)?;
    Ok(surfaces)
}

fn push_menu_surface(
    menu: &AXElement,
    kind: &str,
    id: String,
    deadline: Deadline,
    surfaces: &mut Vec<SurfaceInfo>,
) -> Result<(), AdapterError> {
    let title = read_title(menu, deadline)?;
    let item_count = read_array(menu, "AXChildren", deadline)?.map(|items| items.len());
    surfaces.push(SurfaceInfo {
        id,
        kind: kind.into(),
        title,
        item_count,
        unclassified: Vec::new(),
    });
    Ok(())
}

/// Adds the surface `anchor` is or holds as a direct child: a popover hangs
/// below the control that raised it, not below its window. A control that
/// anchors a surface has few children, so an element with more than
/// `ANCHOR_CHILD_LIMIT` is left unread instead of listing a whole container.
/// The anchor only adds to `surfaces`: a read of it that fails ends the look
/// and leaves what is already listed.
#[cfg(target_os = "macos")]
pub(crate) fn collect_anchored_surfaces(
    anchor: &AXElement,
    deadline: Deadline,
    surfaces: &mut Vec<SurfaceInfo>,
) {
    use core_foundation::{base::TCFType, string::CFString};

    let children = CFString::new("AXChildren");
    let count = crate::tree::properties::ax_ipc::attribute_value_count(
        anchor,
        children.as_concrete_TypeRef(),
        deadline,
    );
    if anchors_few_children(count.ok()) {
        let _ = collect_window_surfaces(anchor, "target", deadline, surfaces);
    }
}

/// The surfaces an action raised: the application's inventory, then the ones
/// hanging below the element the action targeted. Those are read first and
/// cost a few reads, so an inventory that fails does not hide them; the
/// failure is returned only when no anchored surface was found.
pub(crate) fn with_anchored(
    inventory: Result<Vec<SurfaceInfo>, AdapterError>,
    anchored: Vec<SurfaceInfo>,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    match inventory {
        Ok(mut surfaces) => {
            surfaces.extend(anchored);
            Ok(surfaces)
        }
        Err(error) if anchored.is_empty() => Err(error),
        Err(_) => Ok(anchored),
    }
}

fn anchors_few_children(count: Option<usize>) -> bool {
    count.is_some_and(|count| (1..=ANCHOR_CHILD_LIMIT).contains(&count))
}

fn collect_window_surfaces(
    window: &AXElement,
    window_id: &str,
    deadline: Deadline,
    surfaces: &mut Vec<SurfaceInfo>,
) -> Result<(), AdapterError> {
    if let Some(kind) = surface_kind(window, deadline)? {
        surfaces.push(SurfaceInfo {
            id: window_id.to_string(),
            kind: kind.into(),
            title: read_title(window, deadline)?,
            item_count: None,
            unclassified: Vec::new(),
        });
    }
    let Some(children) = read_array(window, "AXChildren", deadline)? else {
        return Ok(());
    };
    for (index, child) in children.iter().enumerate() {
        if let Some(kind) = surface_kind(child, deadline)? {
            surfaces.push(SurfaceInfo {
                id: format!("{window_id}/children/{index}"),
                kind: kind.into(),
                title: read_title(child, deadline)?,
                item_count: None,
                unclassified: Vec::new(),
            });
        }
    }
    Ok(())
}

fn surface_kind(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<&'static str>, AdapterError> {
    let role = read_string(element, "AXRole", deadline)?;
    let subrole = read_string(element, "AXSubrole", deadline)?;
    let dialog = subrole.as_deref() == Some("AXDialog");
    let (modal, minimized) = if dialog {
        (
            crate::tree::surface_read::boolean(element, "AXModal", deadline)?,
            crate::tree::surface_read::boolean(element, "AXMinimized", deadline)?,
        )
    } else {
        (None, None)
    };
    Ok(classify_surface(
        role.as_deref(),
        subrole.as_deref(),
        modal,
        minimized,
    ))
}

/// The kind of surface an element is. `AXDialog` is a window style, not a
/// prompt: a fixed-size ordinary window carries it too. A dialog is an alert
/// only when it is modal, and a minimized window is on the Dock, never in
/// front of the application, so it holds nothing up whatever it carries.
fn classify_surface(
    role: Option<&str>,
    subrole: Option<&str>,
    modal: Option<bool>,
    minimized: Option<bool>,
) -> Option<&'static str> {
    match subrole {
        Some("AXSheet") => Some("sheet"),
        Some("AXPopover") => Some("popover"),
        Some("AXAlert") => Some("alert"),
        Some("AXDialog") if modal == Some(true) && minimized != Some(true) => Some("alert"),
        Some("AXDialog") => None,
        _ => match role {
            Some("AXSheet") => Some("sheet"),
            Some("AXPopover") => Some("popover"),
            _ => None,
        },
    }
}

fn read_title(element: &AXElement, deadline: Deadline) -> Result<Option<String>, AdapterError> {
    match read_string(element, "AXTitle", deadline)? {
        Some(title) => Ok(Some(title)),
        None => read_string(element, "AXDescription", deadline),
    }
}

fn read_string(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<String>, AdapterError> {
    crate::tree::surface_read::string(element, attribute, deadline)
}

fn read_array(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<Vec<AXElement>>, AdapterError> {
    crate::tree::surface_read::elements(element, attribute, deadline).map(Some)
}

fn read_element(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    crate::tree::surface_read::element(element, attribute, deadline)
}

fn ensure_before_deadline(deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::surface_read::ensure_before_deadline(deadline)
}

#[cfg(test)]
#[path = "surface_inventory_tests.rs"]
mod tests;
