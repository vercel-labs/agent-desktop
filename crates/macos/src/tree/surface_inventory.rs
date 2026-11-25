use crate::tree::{AXElement, properties::element::element_for_pid};
use agent_desktop_core::{AdapterError, Deadline, SurfaceInfo};

use super::surface_ids::{
    ChildSurface, ObservedPopover, WindowObservation, push_unique, surface, surface_id,
    window_surfaces,
};

/// The most children an element that anchors a surface has. An element with
/// more is a content container: a list, a table or a column of a browser.
pub(crate) const ANCHOR_CHILD_LIMIT: usize = 16;

/// The surfaces of every window of the process, and its open menus.
pub(crate) fn list_surfaces_for_process(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Deadline,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    let app = element_for_pid(identity.pid());
    super::app_menu::observe(identity, deadline, || {
        let windows = read_array(&app, "AXWindows", deadline)?.unwrap_or_default();
        let listed = windows
            .iter()
            .filter_map(|window| window_number(window, deadline).ok().flatten())
            .collect();
        let popovers =
            crate::system::window::popup_window::shows_unlisted_window(identity.pid(), &listed);
        list_surfaces_once(identity.pid(), deadline, popovers, || Ok(windows))
    })
}

/// The surfaces of one window, and the open menus of its process: what an
/// action on an element of that window can have raised. A popover that opens
/// later is found by `wait --surface`, so the action pays no popover search.
pub(crate) fn list_surfaces_in_window(
    identity: crate::system::process::process_identity::ProcessIdentity,
    window: &AXElement,
    deadline: Deadline,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    super::app_menu::observe(identity, deadline, || {
        list_surfaces_once(identity.pid(), deadline, false, || Ok(vec![window.clone()]))
    })
}

fn list_surfaces_once(
    pid: i32,
    deadline: Deadline,
    popovers: bool,
    windows: impl FnOnce() -> Result<Vec<AXElement>, AdapterError>,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    list_surfaces_with(
        deadline,
        || {
            let mut surfaces = Vec::new();
            for window in windows()? {
                let observed = observe_window(&window, deadline, popovers)?;
                for surface in window_surfaces(observed) {
                    push_unique(&mut surfaces, surface);
                }
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

/// Reads one window, and searches it for a popover when the application shows
/// a window its accessibility window list leaves out.
fn observe_window(
    window: &AXElement,
    deadline: Deadline,
    popovers: bool,
) -> Result<WindowObservation, AdapterError> {
    let number = window_number(window, deadline)?;
    let mut children = Vec::new();
    let listed = read_array(window, "AXChildren", deadline)?.unwrap_or_default();
    for (index, child) in listed.iter().enumerate() {
        let Some(kind) = surface_kind(child, deadline)? else {
            continue;
        };
        children.push(ChildSurface {
            number: window_number(child, deadline)?,
            kind,
            title: read_title(child, deadline)?,
            index,
        });
    }
    let popover = if popovers && !children.iter().any(|child| child.kind == "popover") {
        observe_popover(window, deadline)?
    } else {
        ObservedPopover::Absent
    };
    Ok(WindowObservation {
        number,
        kind: surface_kind(window, deadline)?,
        title: read_title(window, deadline)?,
        children,
        popover,
    })
}

/// The bounded popover search of one window. A search that read every element
/// and found no popover settles an absence; a search cut at its element limit
/// names `popover` unclassified instead, so a popover it could not read is
/// never reported as one the application does not show. Any other failure is
/// the read's own and propagates.
fn observe_popover(
    window: &AXElement,
    deadline: Deadline,
) -> Result<ObservedPopover, AdapterError> {
    match super::surfaces::popover_in_window(window, deadline) {
        Ok(Some(popover)) => Ok(ObservedPopover::Found {
            number: window_number(&popover, deadline)?,
            title: read_title(&popover, deadline)?,
        }),
        Ok(None) => Ok(ObservedPopover::Absent),
        Err(error) if is_incomplete(&error) => Ok(ObservedPopover::Unread {
            title: read_title(window, deadline)?,
        }),
        Err(error) => Err(error),
    }
}

fn window_number(element: &AXElement, deadline: Deadline) -> Result<Option<i64>, AdapterError> {
    let number =
        crate::system::window::window_resolve::ax_window_id_with_deadline(element, deadline)?;
    Ok(number.filter(|number| *number > 0))
}

fn is_incomplete(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .is_some_and(|details| details["complete"] == false)
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
    if !anchors_few_children(count.ok()) {
        return;
    }
    let Ok(listed) = read_array(anchor, "AXChildren", deadline) else {
        return;
    };
    for (index, child) in listed.unwrap_or_default().iter().enumerate() {
        let Ok(Some(kind)) = surface_kind(child, deadline) else {
            continue;
        };
        let Ok(title) = read_title(child, deadline) else {
            continue;
        };
        let id = window_number(child, deadline)
            .ok()
            .flatten()
            .map_or_else(|| format!("target/{kind}/{index}"), surface_id);
        surfaces.push(surface(id, kind, title));
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

fn surface_kind(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<&'static str>, AdapterError> {
    let role = read_string(element, "AXRole", deadline)?;
    let subrole = match role.as_deref() {
        Some("AXWindow") => read_string(element, "AXSubrole", deadline)?,
        _ => None,
    };
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
/// front of the application, so it holds nothing up whatever it carries. A
/// system dialog is the window a system service opens for an application,
/// such as the AirDrop panel of a share, and is always an alert. A dialog
/// subrole belongs to a window, so only a window's subrole is read.
fn classify_surface(
    role: Option<&str>,
    subrole: Option<&str>,
    modal: Option<bool>,
    minimized: Option<bool>,
) -> Option<&'static str> {
    match subrole {
        Some("AXSheet") => Some("sheet"),
        Some("AXPopover") => Some("popover"),
        Some("AXAlert" | "AXSystemDialog") => Some("alert"),
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

fn ensure_before_deadline(deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::surface_read::ensure_before_deadline(deadline)
}

#[cfg(test)]
#[path = "surface_inventory_tests.rs"]
mod tests;
