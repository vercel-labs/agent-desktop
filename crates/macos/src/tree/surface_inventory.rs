use crate::tree::{AXElement, properties::element::element_for_pid};
use agent_desktop_core::{AdapterError, Deadline, SurfaceInfo};

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
            .filter_map(|window| {
                crate::system::window::window_resolve::ax_window_id_with_deadline(window, deadline)
                    .ok()
                    .flatten()
            })
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
            for (index, window) in windows()?.iter().enumerate() {
                let id = format!("windows/{index}");
                let listed = surfaces.len();
                collect_window_surfaces(window, &id, deadline, &mut surfaces)?;
                if popovers && !surfaces[listed..].iter().any(|s| s.kind == "popover") {
                    push_anchored_popover(window, &id, deadline, &mut surfaces)?;
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

/// A headless target window is rarely the focused one, and a popover hangs
/// below the control that raised it, so while the application shows a window
/// its accessibility window list leaves out, every window gets the bounded
/// popover search. A window whose search did not read every element names
/// `popover` unclassified, so a missing popover is not taken for an observed
/// absence.
fn push_anchored_popover(
    window: &AXElement,
    window_id: &str,
    deadline: Deadline,
    surfaces: &mut Vec<SurfaceInfo>,
) -> Result<(), AdapterError> {
    let popover = match super::surfaces::popover_in_window(window, deadline) {
        Ok(popover) => popover,
        Err(error) if is_incomplete(&error) => {
            let title = read_title(window, deadline)?;
            mark_popover_unread(surfaces, window_id, title);
            None
        }
        Err(error) if error.code == agent_desktop_core::ErrorCode::ElementNotFound => None,
        Err(error) => return Err(error),
    };
    if let Some(popover) = popover {
        surfaces.push(SurfaceInfo {
            id: format!("{window_id}/popover"),
            kind: "popover".into(),
            title: read_title(&popover, deadline)?,
            item_count: None,
            unclassified: Vec::new(),
        });
    }
    Ok(())
}

fn is_incomplete(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .is_some_and(|details| details["complete"] == false)
}

fn mark_popover_unread(surfaces: &mut Vec<SurfaceInfo>, window_id: &str, title: Option<String>) {
    match surfaces.iter_mut().find(|entry| entry.id == window_id) {
        Some(entry) => entry.unclassified.push("popover".into()),
        None => surfaces.push(SurfaceInfo {
            id: window_id.to_owned(),
            kind: "window".into(),
            title,
            item_count: None,
            unclassified: vec!["popover".into()],
        }),
    }
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
    let subrole = match role.as_deref() {
        Some("AXWindow") => read_string(element, "AXSubrole", deadline)?,
        _ => None,
    };
    Ok(kind_of(role.as_deref(), subrole.as_deref()))
}

/// A system dialog is the window a system service opens for an application,
/// such as the AirDrop panel of a share. A dialog subrole belongs to a window;
/// a sheet and a popover have their own role, so only a window's subrole is
/// read.
fn kind_of(role: Option<&str>, subrole: Option<&str>) -> Option<&'static str> {
    match (subrole, role) {
        (Some("AXSheet"), _) | (_, Some("AXSheet")) => Some("sheet"),
        (Some("AXPopover"), _) | (_, Some("AXPopover")) => Some("popover"),
        (Some("AXDialog" | "AXSystemDialog" | "AXAlert"), _) => Some("alert"),
        _ => None,
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
