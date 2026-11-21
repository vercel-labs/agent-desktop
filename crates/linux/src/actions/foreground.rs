//! Which headless actions take the foreground on Mutter. Opening a GTK 4
//! popover, drop down or menu button that offers no `Selection` interface
//! moves the keyboard into the popup (L-W0-12); `ShowMenu` opens a menu over
//! the user's window; a GTK 4 or Qt control that opens a dialog activates it
//! (wave0 C3); `Component.GrabFocus` raises and activates a GTK 3 window
//! (L-W0-17), so focusing an element takes the foreground on every toolkit
//! here. GTK 3 popover toggles and dialog openers stay below the user's
//! window (L-W0-13, C3) and are not refused. Core denies a request whose policy
//! forbids that foreground before it checks anything else.

use agent_desktop_core::{
    Action, ActionRequest, AdapterError, Deadline, HeadedRequirement, NativeHandle,
};

use super::context::Target;
use super::select::advertises_selection;
use super::select::select_popup::{displayed_value, value_already_shown};
use crate::system::SharedBus;
use crate::tree::atspi_element;
use crate::tree::capabilities::action_read::{NodeView, read_node_view};
use crate::tree::live_read::live_offscreen::toolkit_of;
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind};
use crate::tree::vocabulary::state_bits::HAS_POPUP;

const SHOW_MENU: &str = "ShowMenu";

pub(crate) fn headed_requirement(
    bus: &SharedBus,
    handle: &NativeHandle,
    request: &ActionRequest,
    deadline: Deadline,
) -> Result<HeadedRequirement, AdapterError> {
    let opens = matches!(
        request.action,
        Action::Click
            | Action::RightClick
            | Action::Toggle
            | Action::Check
            | Action::Uncheck
            | Action::Expand
            | Action::Select(_)
    );
    let Ok(element) = atspi_element(handle) else {
        return Ok(request.headed_requirement());
    };
    if request.policy.is_headed() {
        return Ok(request.headed_requirement());
    }
    if matches!(request.action, Action::SetFocus) {
        return Ok(HeadedRequirement::FocusedApplication);
    }
    if !opens {
        return Ok(request.headed_requirement());
    }
    let view = read_node_view(bus, handle, deadline)?;
    let toolkit = kind(&toolkit_of(bus, element, deadline));
    let owner = || {
        let ancestry = element.ancestry();
        ancestry
            .combo_owner()
            .or_else(|| ancestry.button_parent())
            .map(|owner| read_node_view(bus, &owner.into_native_handle(), deadline))
            .transpose()
    };
    let target = Target {
        bus,
        handle,
        element,
        deadline,
    };
    let takes = takes_foreground(&request.action, &view, toolkit, owner)?
        && !already_selected(&target, &request.action);
    Ok(if takes {
        HeadedRequirement::FocusedApplication
    } else {
        request.headed_requirement()
    })
}

/// A drop down that already shows the wanted choice is answered without
/// opening anything, and `select_in_popup` opens one only when the policy lets
/// it take the keyboard, so that select takes no foreground.
fn already_selected(target: &Target<'_>, action: &Action) -> bool {
    match action {
        Action::Select(value) => value_already_shown(displayed_value(target), value),
        _ => false,
    }
}

/// The toggle inside a drop down or a menu button carries no popup state of
/// its own; `owner` reads the combo or button that holds it, only when the
/// element itself does not already decide. A field the read could not answer
/// counts as taking the foreground: unread evidence never proves an action
/// safe.
fn takes_foreground(
    action: &Action,
    view: &NodeView,
    toolkit: ToolkitKind,
    owner: impl FnOnce() -> Result<Option<NodeView>, AdapterError>,
) -> Result<bool, AdapterError> {
    if view.action_names().is_none_or(shows_menu) {
        return Ok(true);
    }
    if toolkit == ToolkitKind::Gtk3 {
        return Ok(false);
    }
    if opens_dialog(action, view.name_read()) || opens_popup_without_selection(view) {
        return Ok(true);
    }
    Ok(owner()?.is_some_and(|owner| opens_popup_without_selection(&owner)))
}

fn shows_menu(names: &[String]) -> bool {
    names
        .iter()
        .any(|name| name.eq_ignore_ascii_case(SHOW_MENU))
}

/// Owner-judgment rule: a label that ends in an ellipsis is the convention
/// GTK and Qt applications share for a control that opens a dialog. It is a
/// predictor, not a proof; nothing in AT-SPI says a control opens a window.
fn opens_dialog(action: &Action, name: Option<Option<String>>) -> bool {
    let activates = matches!(
        action,
        Action::Click | Action::Toggle | Action::Check | Action::Uncheck
    );
    activates
        && name.is_none_or(|name| {
            name.as_deref()
                .map(str::trim_end)
                .is_some_and(|name| name.ends_with('…') || name.ends_with("..."))
        })
}

fn opens_popup_without_selection(view: &NodeView) -> bool {
    view.state(HAS_POPUP) != Some(false) && !advertises_selection(view)
}

#[cfg(test)]
#[path = "foreground_tests.rs"]
mod tests;
