//! Keyboard input for an element that already holds its application's
//! keyboard focus. The key events go to the application's process only: the
//! application is not activated, and the user's own keyboard input is not
//! touched. Nothing is sent unless the element is the focus holder, and that
//! is confirmed again before every chunk and every key, so nothing lands in
//! another element.

use agent_desktop_core::{AdapterError, Deadline, KeyCombo};

use crate::actions::physical_keyboard::{
    focused_element_matches, lost_focus, owning_process, require_same_process,
};
use crate::tree::AXElement;
use crate::tree::properties::attributes;

/// Whether `element` is the element its application reports as its keyboard
/// focus. The application does not have to be active.
pub(crate) fn holds_focus(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    match crate::system::app::app_ops::pid_from_element(element, deadline).filter(|pid| *pid > 0) {
        Some(pid) => focused_element_matches(element, pid, deadline),
        None => Ok(false),
    }
}

/// The text element that holds the keyboard focus of `pid`'s application.
pub(crate) fn text_focus_holder(
    pid: i32,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    attributes::set_messaging_timeout(&app, deadline)?;
    let holder = attributes::copy_element_attr_result(&app, "AXFocusedUIElement", deadline);
    deadline.ensure_remaining()?;
    let Ok(Some(holder)) = holder else {
        return Ok(None);
    };
    let role = attributes::copy_string_attr_result(&holder, "AXRole", deadline);
    deadline.ensure_remaining()?;
    let role = role.ok().flatten();
    Ok(crate::actions::type_text::takes_typed_text(role.as_deref()).then_some(holder))
}

/// Types `text` into `element`. Every chunk goes only while `element` holds
/// its application's keyboard focus: an application that moves the edit to
/// another element, or any other change of the focus holder, stops the typing.
pub(crate) fn type_text(
    element: &AXElement,
    text: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    crate::input::keyboard::preflight_text(text, deadline)?;
    let identity = owning_process(element, deadline)?;
    crate::input::keyboard::synthesize_text(text, identity.pid(), deadline, |deadline| {
        require_same_process(identity)?;
        if focused_element_matches(element, identity.pid(), deadline)? {
            return Ok(());
        }
        Err(lost_focus())
    })
}

/// Sends `combo` to `element`, which must hold the keyboard focus.
pub(crate) fn press(
    element: &AXElement,
    combo: &KeyCombo,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let identity = owning_process(element, deadline)?;
    require_same_process(identity)?;
    if !focused_element_matches(element, identity.pid(), deadline)? {
        return Err(lost_focus());
    }
    crate::input::keyboard::synthesize_key(combo, Some(identity.pid()), deadline)
}
