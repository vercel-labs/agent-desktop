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
    let own_route = is_popup_dropdown(target, &view)? || is_pointer_combo(&view);
    let mut reader = BusReader {
        bus: target.bus,
        deadline: target.deadline,
        inspected_role: None,
    };
    decide(own_route, || {
        search(&mut reader, target.element, display_value)
    })
}

/// A popup or pointer-combo select has its own route and is never refused
/// here, so its search does not even run.
fn decide<N>(
    own_route: bool,
    find: impl FnOnce() -> Result<Search<N>, AdapterError>,
) -> Result<(), AdapterError> {
    if own_route {
        return Ok(());
    }
    refusal_for(&find()?)
}

fn refusal_for<N>(found: &Search<N>) -> Result<(), AdapterError> {
    match found {
        Search::NotFound { selectable: false } => Err(no_route()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{DeliverySemantics, ErrorCode};

    #[test]
    fn a_search_with_no_selection_route_refuses_before_any_scroll() {
        let error = refusal_for::<()>(&Search::NotFound { selectable: false }).unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }

    #[test]
    fn a_popup_or_pointer_combo_route_is_never_refused_and_never_searched() {
        let result = decide::<()>(true, || panic!("the search must not run"));
        assert!(result.is_ok());
        let refused = decide::<()>(false, || Ok(Search::NotFound { selectable: false }));
        assert!(refused.is_err());
    }

    #[test]
    fn every_other_search_outcome_is_left_to_the_select_itself() {
        assert!(refusal_for::<()>(&Search::NotFound { selectable: true }).is_ok());
        assert!(refusal_for::<()>(&Search::Ambiguous).is_ok());
        assert!(refusal_for::<()>(&Search::Incomplete).is_ok());
    }
}
