use super::{ClickRoute, ClickTraits, click_route};
use crate::actions::disclosure::ExpandKind;

fn traits(invoke: bool, button: bool, state: Option<ExpandKind>) -> ClickTraits {
    ClickTraits {
        invoke_available: invoke,
        is_button: button,
        expand_state: state,
    }
}

#[test]
fn a_collapsed_button_without_invoke_is_clicked_by_expanding_it() {
    assert_eq!(
        click_route(traits(false, true, Some(ExpandKind::Collapsed))),
        ClickRoute::Disclose {
            want_expanded: true
        }
    );
}

#[test]
fn an_expanded_button_without_invoke_is_clicked_by_collapsing_it() {
    assert_eq!(
        click_route(traits(false, true, Some(ExpandKind::Expanded))),
        ClickRoute::Disclose {
            want_expanded: false
        }
    );
}

#[test]
fn a_button_that_can_be_invoked_keeps_the_click_chain() {
    assert_eq!(
        click_route(traits(true, true, Some(ExpandKind::Collapsed))),
        ClickRoute::Chain
    );
}

#[test]
fn a_non_button_with_expand_collapse_keeps_the_click_chain() {
    assert_eq!(
        click_route(traits(false, false, Some(ExpandKind::Collapsed))),
        ClickRoute::Chain
    );
}

#[test]
fn an_unknown_partial_or_leaf_state_keeps_the_click_chain() {
    for state in [
        None,
        Some(ExpandKind::PartiallyExpanded),
        Some(ExpandKind::LeafNode),
    ] {
        assert_eq!(click_route(traits(false, true, state)), ClickRoute::Chain);
    }
}
