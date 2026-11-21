use agent_desktop_core::Deadline;

use crate::tree::AXElement;

/// Reads whether a menu control's own menu is open: a visible `AXMenu` child
/// with nonempty bounds. `None` when the element is not a menu control or a
/// child could not be read.
pub(crate) fn owned_menu_open(element: &AXElement, deadline: Deadline) -> Option<bool> {
    use crate::tree::surface_read;
    let end = deadline;
    let role = surface_read::string(element, "AXRole", end).ok()??;
    if !matches!(
        role.as_str(),
        "AXMenuButton" | "AXPopUpButton" | "AXComboBox" | "AXMenuBarItem" | "AXMenuItem"
    ) {
        return None;
    }
    let mut complete = true;
    for child in surface_read::elements(element, "AXChildren", end).ok()? {
        if surface_read::string(&child, "AXRole", end).ok()?? != "AXMenu" {
            continue;
        }
        let visible = surface_read::boolean(&child, "AXVisible", end).ok()?;
        let hidden = surface_read::boolean(&child, "AXHidden", end).ok()?;
        let bounds = crate::tree::element_bounds::read_bounds_with_deadline(&child, end).ok()?;
        match menu_visibility(visible, hidden, bounds) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => complete = false,
        }
    }
    complete.then_some(false)
}

fn menu_visibility(
    visible: Option<bool>,
    hidden: Option<bool>,
    bounds: Option<agent_desktop_core::Rect>,
) -> Option<bool> {
    if visible == Some(false) || hidden == Some(true) {
        return Some(false);
    }
    let bounds = bounds?;
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Some(false);
    }
    (visible == Some(true) || hidden == Some(false)).then_some(true)
}

#[cfg(test)]
#[path = "owned_menu_tests.rs"]
mod tests;
