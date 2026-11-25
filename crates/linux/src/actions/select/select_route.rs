use agent_desktop_core::AdapterError;

use super::select_pointer_combo::is_pointer_combo;
use super::select_popup::is_popup_dropdown;
use super::select_search::{Search, search};
use super::{BusReader, no_route};
use crate::actions::context::Target;

/// The part of `select` that decides, by reading alone, that the target has
/// no selection route. Any other outcome is left to `select` itself.
pub(crate) fn refuse_unroutable(
    target: &Target<'_>,
    display_value: &str,
) -> Result<(), AdapterError> {
    let view = target.view()?;
    if is_popup_dropdown(target, &view)? || is_pointer_combo(&view) {
        return Ok(());
    }
    let mut reader = BusReader {
        bus: target.bus,
        deadline: target.deadline,
        inspected_role: None,
    };
    match search(&mut reader, target.element, display_value)? {
        Search::NotFound { selectable: false } => Err(no_route()),
        _ => Ok(()),
    }
}
