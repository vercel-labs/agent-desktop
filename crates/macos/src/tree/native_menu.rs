use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use crate::tree::{AXElement, surface_read};

pub(crate) fn is_native_menu(
    element: &AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let end = deadline;
    classify(
        element.clone(),
        deadline,
        |element| surface_read::string(element, "AXRole", end),
        |element| surface_read::element(element, "AXParent", end),
        |application, menubar| {
            Ok(surface_read::element(application, "AXMenuBar", end)?
                .is_some_and(|owned| crate::tree::same_element(&owned, menubar)))
        },
    )
}

/// A press on this element reaches a menu: the application's menu bar
/// hierarchy, or an item of any open menu (a pop-up button's, a context
/// menu's). Such an item publishes no `AXWindow`, so the only delivery that
/// reaches it is `AXPress`; a physical click has no window to verify.
pub(crate) fn presses_like_a_menu(
    element: &AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let end = deadline;
    Ok(classify_menu_item(
        element.clone(),
        deadline,
        |element| surface_read::string(element, "AXRole", end),
        |element| surface_read::element(element, "AXParent", end),
    )? || is_native_menu(element, deadline)?)
}

fn classify_menu_item<T>(
    mut element: T,
    deadline: Deadline,
    mut role: impl FnMut(&T) -> Result<Option<String>, AdapterError>,
    mut parent: impl FnMut(&T) -> Result<Option<T>, AdapterError>,
) -> Result<bool, AdapterError> {
    if role(&element)?.as_deref() != Some("AXMenuItem") {
        return Ok(false);
    }
    let mut in_menu = false;
    for _ in 0..64 {
        deadline.ensure_remaining()?;
        let Some(next) = parent(&element)? else {
            return Ok(in_menu);
        };
        match role(&next)?.as_deref() {
            Some("AXWebArea") => return Ok(false),
            Some("AXMenu") => in_menu = true,
            Some("AXApplication") => return Ok(in_menu),
            _ => {}
        }
        element = next;
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Menu item ancestry exceeded its observation budget",
    )
    .with_details(serde_json::json!({ "complete": false, "kind": "menu_ancestry_limit" })))
}

fn classify<T: Clone>(
    mut element: T,
    deadline: Deadline,
    mut role: impl FnMut(&T) -> Result<Option<String>, AdapterError>,
    mut parent: impl FnMut(&T) -> Result<Option<T>, AdapterError>,
    mut owns_menubar: impl FnMut(&T, &T) -> Result<bool, AdapterError>,
) -> Result<bool, AdapterError> {
    let mut menubar = None;
    for depth in 0..64 {
        deadline.ensure_remaining()?;
        let role = role(&element)?;
        match role.as_deref() {
            Some("AXMenuItem" | "AXMenuBarItem") => {}
            Some("AXMenu") if depth > 0 => {}
            Some("AXMenuBar") if depth > 0 => menubar = Some(element.clone()),
            Some("AXApplication") => {
                return menubar
                    .as_ref()
                    .map_or(Ok(false), |menubar| owns_menubar(&element, menubar));
            }
            _ => return Ok(false),
        }
        let Some(next) = parent(&element)? else {
            return Ok(false);
        };
        element = next;
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Native menu ancestry exceeded its observation budget",
    )
    .with_details(serde_json::json!({ "complete": false, "kind": "menu_ancestry_limit" })))
}

#[cfg(test)]
#[path = "native_menu_tests.rs"]
mod tests;
