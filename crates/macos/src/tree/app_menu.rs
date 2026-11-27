use super::{AXElement, properties::element::element_for_pid, surface_read};
use agent_desktop_core::{AdapterError, Deadline, ErrorCode, SnapshotSurface};

const MAX_SURFACE_NODES: usize = 2_048;

pub(crate) fn observe<T>(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Deadline,
    mut operation: impl FnMut() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    surface_read::ensure_before_deadline(deadline)?;
    let mut reacquired = false;
    agent_desktop_core::refs::resolve_retry::retry_incomplete_until(deadline, || {
        reacquire_once(deadline, &mut reacquired, || {
            require_generation(identity)?;
            let result = operation();
            require_generation(identity)?;
            result
        })
    })
}

pub(crate) fn require_generation(
    identity: crate::system::process::process_identity::ProcessIdentity,
) -> Result<(), AdapterError> {
    if identity.still_matches()? {
        Ok(())
    } else {
        Err(AdapterError::stale_ref_because(
            "Application process instance changed during observation",
        ))
    }
}

fn reacquire_once<T>(
    deadline: Deadline,
    reacquired: &mut bool,
    mut operation: impl FnMut() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    loop {
        surface_read::ensure_before_deadline(deadline)?;
        match operation() {
            Err(mut error) if invalidated(&error) => {
                if !*reacquired {
                    *reacquired = true;
                    continue;
                }
                error.code = ErrorCode::AppUnresponsive;
                let mut details = error
                    .details
                    .take()
                    .unwrap_or_else(|| serde_json::json!({}));
                if let Some(fields) = details.as_object_mut() {
                    fields.insert("complete".into(), false.into());
                    fields.insert("retryable".into(), false.into());
                    fields.insert("menu_root_reacquired".into(), true.into());
                }
                return Err(error.with_details(details));
            }
            result => return result,
        }
    }
}

fn invalidated(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details["ax_error"].as_i64())
        == Some(i64::from(accessibility_sys::kAXErrorInvalidUIElement))
}

pub(crate) fn root_for_pid(
    pid: i32,
    surface: SnapshotSurface,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    match surface {
        SnapshotSurface::Menubar => menubar_for_pid(pid, deadline),
        SnapshotSurface::Menu => menu_element_for_pid(pid, deadline),
        _ => Err(AdapterError::not_supported("native menu surface")),
    }
}

pub(super) fn require_root<T>(
    read: Result<Option<T>, AdapterError>,
    surface: SnapshotSurface,
    message: &str,
) -> Result<T, AdapterError> {
    read?.ok_or_else(|| AdapterError::surface_absent(surface, message))
}

#[cfg(target_os = "macos")]
fn menubar_for_pid(pid: i32, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    let Some(menubar) = surface_read::element(&app, "AXMenuBar", deadline)? else {
        return Ok(None);
    };
    if !has_role(&menubar, "AXMenuBar", deadline)? {
        return Err(invalid_root_error());
    }
    Ok(Some(menubar))
}

#[cfg(target_os = "macos")]
fn menu_element_for_pid(pid: i32, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    Ok(open_menus_for_pid(pid, deadline)?
        .into_iter()
        .next()
        .map(|(_, _, menu)| menu))
}

pub(crate) fn open_menus_for_pid(
    pid: i32,
    deadline: Deadline,
) -> Result<Vec<(&'static str, String, AXElement)>, AdapterError> {
    open_menus_for_pid_until(pid, Transient::Menu, deadline)
}

fn open_menus_for_pid_until(
    pid: i32,
    accept: Transient,
    deadline: Deadline,
) -> Result<Vec<(&'static str, String, AXElement)>, AdapterError> {
    let mut menus = Vec::new();
    if let Some(menubar) = menubar_for_pid(pid, deadline)? {
        menus.extend(
            open_menubar_menus_with(
                &menubar,
                |element| surface_read::elements(element, "AXChildren", deadline),
                |element| {
                    Ok(has_role(element, "AXMenuBarItem", deadline)?
                        && surface_read::boolean(element, "AXSelected", deadline)? == Some(true))
                },
                |element| has_role(element, "AXMenu", deadline),
            )?
            .into_iter()
            .map(|(id, menu)| ("menu", id, menu)),
        );
    }
    if menus.is_empty()
        && let Some((id, menu)) = surface_from_app(pid, accept, deadline)?
    {
        menus.push(("context_menu", id, menu));
    }
    Ok(menus)
}

/// What a search below the application accepts as the open transient surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transient {
    Menu,
    MenuOrPopover,
}

/// The menu or popover the application shows, in one pass: an open menu bar
/// menu first, then the bounded search below the application.
#[cfg(target_os = "macos")]
pub(crate) fn open_transient_surface(
    pid: i32,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    Ok(
        open_menus_for_pid_until(pid, Transient::MenuOrPopover, deadline)?
            .into_iter()
            .next()
            .map(|(_, _, surface)| surface),
    )
}

fn open_menubar_menus_with<T>(
    menubar: &T,
    mut children: impl FnMut(&T) -> Result<Vec<T>, AdapterError>,
    mut selected: impl FnMut(&T) -> Result<bool, AdapterError>,
    mut is_menu: impl FnMut(&T) -> Result<bool, AdapterError>,
) -> Result<Vec<(String, T)>, AdapterError> {
    let mut menus = Vec::new();
    for (item_index, item) in children(menubar)?.into_iter().enumerate() {
        if !selected(&item)? {
            continue;
        }
        for (child_index, child) in children(&item)?.into_iter().enumerate() {
            if is_menu(&child)? {
                menus.push((
                    format!("app/menubar/children/{item_index}/children/{child_index}"),
                    child,
                ));
            }
        }
    }
    Ok(menus)
}

#[cfg(target_os = "macos")]
fn surface_from_app(
    pid: i32,
    accept: Transient,
    deadline: Deadline,
) -> Result<Option<(String, AXElement)>, AdapterError> {
    let app = element_for_pid(pid);
    for (index, menu) in surface_read::elements(&app, "AXMenus", deadline)?
        .into_iter()
        .enumerate()
    {
        if displayed_menu(&menu, deadline)? {
            return Ok(Some((format!("app/menus/{index}"), menu)));
        }
    }
    if let Some(focused) = surface_read::element(&app, "AXFocusedUIElement", deadline)?
        && let Some(menu) =
            find_menu_descendant(focused, "app/focused-element".into(), accept, deadline)?
    {
        return Ok(Some(menu));
    }
    for (index, child) in surface_read::elements(&app, "AXChildren", deadline)?
        .into_iter()
        .enumerate()
    {
        if let Some(menu) =
            find_menu_descendant(child, format!("app/children/{index}"), accept, deadline)?
        {
            return Ok(Some(menu));
        }
    }
    Ok(None)
}

/// How far a surface search opens the elements below its root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChildExpansion {
    /// A menu hangs directly below the element that shows it, so an element
    /// with more than `ANCHOR_CHILD_LIMIT` children is a content container and
    /// stays closed: reading its items made the search grow with the content
    /// of every window.
    AnchorsOnly,
    /// A popover can hang below a control inside a list or a table row, so
    /// every element is opened. `MAX_SURFACE_NODES` and the deadline still
    /// bound the search, and reaching the limit is reported as incomplete.
    EveryNode,
}

impl ChildExpansion {
    fn opens(self, children: usize) -> bool {
        match self {
            Self::AnchorsOnly => children <= super::surface_inventory::ANCHOR_CHILD_LIMIT,
            Self::EveryNode => true,
        }
    }
}

#[cfg(target_os = "macos")]
pub(super) fn find_menu_descendant(
    root: AXElement,
    source: String,
    accept: Transient,
    deadline: Deadline,
) -> Result<Option<(String, AXElement)>, AdapterError> {
    let expansion = match accept {
        Transient::Menu => ChildExpansion::AnchorsOnly,
        Transient::MenuOrPopover => ChildExpansion::EveryNode,
    };
    find_surface_descendant_with(root, source, expansion, deadline, |element, expand| {
        let role = surface_read::string(element, "AXRole", deadline)?;
        if role.as_deref() == Some("AXMenuBar") {
            return Ok((false, Vec::new()));
        }
        if accept == Transient::MenuOrPopover
            && (role.as_deref() == Some("AXPopover")
                || surface_read::string(element, "AXSubrole", deadline)?.as_deref()
                    == Some("AXPopover"))
        {
            return Ok((true, Vec::new()));
        }
        if role.as_deref() == Some("AXMenu")
            && surface_read::boolean(element, "AXVisible", deadline)? != Some(false)
        {
            return Ok((true, Vec::new()));
        }
        if !expand {
            return Ok((false, Vec::new()));
        }
        Ok((
            false,
            surface_read::elements(element, "AXChildren", deadline)?,
        ))
    })
}

/// Breadth-first search below `root` for a transient surface: a menu, or a
/// popover. `expansion` decides how far the search opens the tree, because a
/// menu and a popover hang differently: a menu sits directly below its anchor,
/// while a popover can sit below a control inside a list or a table row.
/// `inspect` gets whether it may return the element's children.
pub(super) fn find_surface_descendant_with<T>(
    root: T,
    source: String,
    expansion: ChildExpansion,
    deadline: Deadline,
    mut inspect: impl FnMut(&T, bool) -> Result<(bool, Vec<T>), AdapterError>,
) -> Result<Option<(String, T)>, AdapterError> {
    let mut pending = std::collections::VecDeque::from([(source, root, true)]);
    let mut incomplete = None;
    let mut visited = 0_usize;
    while let Some((path, element, expand)) = pending.pop_front() {
        surface_read::ensure_before_deadline(deadline)?;
        visited += 1;
        if visited > MAX_SURFACE_NODES {
            return Err(surface_limit_error());
        }
        let (is_menu, children) = match inspect(&element, expand) {
            Ok(observed) => observed,
            Err(error) if error.code == ErrorCode::AppUnresponsive && !invalidated(&error) => {
                incomplete.get_or_insert(error);
                continue;
            }
            Err(error) => return Err(error),
        };
        if is_menu {
            return Ok(Some((path, element)));
        }
        let expand_children = expansion.opens(children.len());
        pending.extend(
            children
                .into_iter()
                .enumerate()
                .map(|(index, child)| (format!("{path}/children/{index}"), child, expand_children)),
        );
    }
    incomplete.map_or(Ok(None), Err)
}

#[cfg(target_os = "macos")]
fn displayed_menu(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    Ok(has_role(element, "AXMenu", deadline)?
        && surface_read::boolean(element, "AXVisible", deadline)? != Some(false))
}

#[cfg(target_os = "macos")]
fn has_role(element: &AXElement, expected: &str, deadline: Deadline) -> Result<bool, AdapterError> {
    Ok(surface_read::string(element, "AXRole", deadline)?.as_deref() == Some(expected))
}

fn invalid_root_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Application returned an invalid menu bar root",
    )
    .with_details(serde_json::json!({
        "kind": "surface_root_invalid", "surface": "menubar", "complete": false,
        "retryable": false,
    }))
}

/// Whether a surface search stopped at its node budget, which means a surface
/// may still sit below the cut rather than being absent.
pub(crate) fn is_surface_search_limit(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .is_some_and(|details| details["kind"] == "surface_search_limit")
}

fn surface_limit_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Accessibility surface search exceeded its node budget",
    )
    .with_details(serde_json::json!({
        "kind": "surface_search_limit",
        "limit": MAX_SURFACE_NODES,
        "complete": false,
        "retryable": false,
    }))
    .with_suggestion("Retry with the target application in a more stable UI state")
}

#[cfg(test)]
#[path = "app_menu_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "app_menu_search_tests.rs"]
mod search_tests;
