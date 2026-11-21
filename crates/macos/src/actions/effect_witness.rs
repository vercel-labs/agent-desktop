//! The effect witness of an element on macOS: what an action on it can change
//! outside its own state. Every part reads through the existing accessibility
//! reads; a part that cannot be read stays `None`.

#[cfg(target_os = "macos")]
mod imp {
    use std::hash::{Hash, Hasher};

    use agent_desktop_core::{
        AdapterError, Deadline, EffectWitness, FocusWitness, TargetWitness, WindowWitness,
    };

    use crate::tree::{AXElement, properties::attributes, surface_read};

    const FOCUS_DESCENT_WALK: usize = 3;

    /// The target and focus parts: what a press changes inside the target's
    /// application. Fails as a stale reference when the target is gone.
    pub(crate) fn read_local(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<EffectWitness, AdapterError> {
        let role = surface_read::string(element, "AXRole", deadline)?;
        let app = crate::system::app::app_ops::pid_from_element(element, deadline)
            .map(crate::tree::element_for_pid);
        let focus = match app.map(|app| {
            attributes::copy_element_attr_answered_result(&app, "AXFocusedUIElement", deadline)
        }) {
            Some(Ok(Some(focused))) => focus_of(&focused, element, deadline),
            Some(Ok(None)) => FocusWitness::nothing_focused(),
            Some(Err(_)) | None => FocusWitness::default(),
        };
        Ok(EffectWitness {
            target: TargetWitness {
                selected: attributes::copy_bool_attr_result(element, "AXSelected", deadline)
                    .ok()
                    .flatten(),
                value: crate::tree::copy_value_typed(element, deadline),
                surfaces: surfaces_below(element, role.as_deref(), deadline),
            },
            focus,
            window: WindowWitness::default(),
        })
    }

    /// All parts: the target and focus parts, and the target's window.
    pub(crate) fn read(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<EffectWitness, AdapterError> {
        let mut witness = read_local(element, deadline)?;
        witness.window = window_of(element, deadline);
        Ok(witness)
    }

    /// Open menus and popovers directly below the element: a menu control's
    /// own menu, or a popover a control anchors. `None` when a menu or a
    /// child's role could not be read: an uncounted surface would make a later
    /// whole read look like a surface the action opened.
    fn surfaces_below(element: &AXElement, role: Option<&str>, deadline: Deadline) -> Option<u32> {
        use crate::actions::owned_menu::{is_menu_control, menu_open};
        if role.is_some_and(is_menu_control) {
            return menu_open(element, deadline).map(u32::from);
        }
        let children = surface_read::elements(element, "AXChildren", deadline).ok()?;
        if children.len() > crate::tree::surface_inventory::ANCHOR_CHILD_LIMIT {
            return None;
        }
        super::popover_count(
            children
                .iter()
                .map(|child| surface_read::string(child, "AXRole", deadline)),
        )
    }

    fn focus_of(focused: &AXElement, element: &AXElement, deadline: Deadline) -> FocusWitness {
        FocusWitness {
            element: crate::tree::resolve::retained::capture(focused)
                .ok()
                .flatten(),
            inside_target: Some(focus_reached(focused, element, deadline)),
            in_target_window: shares_window(focused, element, deadline),
            value: crate::tree::copy_value_typed(focused, deadline),
        }
    }

    fn window_of(element: &AXElement, deadline: Deadline) -> WindowWitness {
        let window = surface_read::element(element, "AXWindow", deadline)
            .ok()
            .flatten();
        let windows = crate::system::app::app_ops::pid_from_element(element, deadline)
            .map(crate::tree::element_for_pid)
            .and_then(|app| {
                crate::tree::resolve::resolve_ax_read::read_array(&app, "AXWindows", deadline)
                    .ok()
                    .flatten()
            });
        WindowWitness {
            title: window
                .as_ref()
                .and_then(|window| title_of(window, deadline)),
            content: window
                .as_ref()
                .and_then(|window| content_sign(window, deadline)),
            windows: windows
                .as_ref()
                .and_then(|windows| u32::try_from(windows.len()).ok()),
            present: window
                .as_ref()
                .zip(windows.as_ref())
                .map(|(window, windows)| {
                    windows
                        .iter()
                        .any(|listed| crate::tree::same_element(listed, window))
                }),
        }
    }

    /// The title with the document, which a document window names apart.
    fn title_of(window: &AXElement, deadline: Deadline) -> Option<String> {
        let title = surface_read::string(window, "AXTitle", deadline)
            .ok()
            .flatten();
        let document = surface_read::string(window, "AXDocument", deadline)
            .ok()
            .flatten();
        (title.is_some() || document.is_some()).then(|| {
            format!(
                "{}\n{}",
                title.unwrap_or_default(),
                document.unwrap_or_default()
            )
        })
    }

    /// A sign of the window's content: the role and the number of children of
    /// each direct child. A pane that a tab swaps in has another shape.
    /// The child count is read without copying the children.
    fn content_sign(window: &AXElement, deadline: Deadline) -> Option<u64> {
        use core_foundation::{base::TCFType, string::CFString};
        let children = CFString::new("AXChildren");
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for child in surface_read::elements(window, "AXChildren", deadline).ok()? {
            surface_read::string(&child, "AXRole", deadline)
                .ok()
                .flatten()
                .hash(&mut hasher);
            crate::tree::properties::ax_ipc::attribute_value_count(
                &child,
                children.as_concrete_TypeRef(),
                deadline,
            )
            .ok()
            .hash(&mut hasher);
        }
        Some(hasher.finish())
    }

    /// A control that owns an editor hands focus to the editor, not to itself:
    /// a PDF form field in Preview publishes a text area and focus lands there.
    fn focus_reached(focused: &AXElement, element: &AXElement, deadline: Deadline) -> bool {
        let mut current = focused.clone();
        for _ in 0..FOCUS_DESCENT_WALK {
            if crate::tree::same_element(&current, element) {
                return true;
            }
            let Some(parent) = attributes::copy_element_attr_result(&current, "AXParent", deadline)
                .ok()
                .flatten()
            else {
                return false;
            };
            current = parent;
        }
        false
    }

    fn shares_window(focused: &AXElement, element: &AXElement, deadline: Deadline) -> Option<bool> {
        let window = |el| attributes::copy_element_attr_result(el, "AXWindow", deadline);
        Some(crate::tree::same_element(
            &window(focused).ok()??,
            &window(element).ok()??,
        ))
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::{read, read_local};

/// The popovers among children's roles, or `None` when a role read failed. A
/// child with no role is no popover.
fn popover_count<E>(roles: impl Iterator<Item = Result<Option<String>, E>>) -> Option<u32> {
    let mut count = 0;
    for role in roles {
        if role.ok()?.as_deref() == Some("AXPopover") {
            count += 1;
        }
    }
    Some(count)
}

#[cfg(test)]
#[path = "effect_witness_tests.rs"]
mod tests;
