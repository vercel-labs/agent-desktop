use super::{frame_listed, may_own_popup, surfaces_of};
use crate::tree::AtspiElement;
use crate::tree::test_support::helpers::{read_of, words};
use crate::tree::vocabulary::role_numbers::{COMBO_BOX, LIST, MENU};
use crate::tree::vocabulary::state_bits::{EXPANDED, HAS_POPUP, SHOWING, StateBits};
use crate::tree::{Field, ObservedRead, WalkedNode};

fn node(
    role: u32,
    states: &[u32],
    children: Vec<WalkedNode<ObservedRead>>,
) -> WalkedNode<ObservedRead> {
    let mut read = read_of(role, "n", &[]);
    read.node.traits.states = Field::Present(words(states));
    WalkedNode {
        payload: read,
        children,
        truncated: false,
        child_count: None,
        source_index: 0,
        in_web_document: false,
    }
}

#[test]
fn the_surfaces_of_a_target_are_the_popups_that_show_below_it() {
    let shown = || node(MENU, &[SHOWING], Vec::new());
    let hidden = || node(MENU, &[], Vec::new());
    assert_eq!(
        surfaces_of(&node(COMBO_BOX, &[], vec![shown(), shown()])),
        Some(2)
    );
    assert_eq!(surfaces_of(&node(COMBO_BOX, &[], vec![hidden()])), Some(0));
    assert_eq!(
        surfaces_of(&node(COMBO_BOX, &[EXPANDED], vec![hidden()])),
        Some(1)
    );
    let mut unread = node(COMBO_BOX, &[], Vec::new());
    unread.payload.node.traits.states = Field::Failed("gone".to_owned());
    assert_eq!(surfaces_of(&unread), None);
}

#[test]
fn a_popup_count_that_missed_a_menu_is_unknown() {
    let mut menu_unread = node(MENU, &[], Vec::new());
    menu_unread.payload.node.traits.states = Field::Failed("gone".to_owned());
    assert_eq!(surfaces_of(&node(COMBO_BOX, &[], vec![menu_unread])), None);

    let mut cut = node(COMBO_BOX, &[], vec![node(MENU, &[], Vec::new())]);
    cut.children[0].truncated = true;
    assert_eq!(surfaces_of(&cut), None);

    let mut rows_cut = node(COMBO_BOX, &[], vec![node(LIST, &[], Vec::new())]);
    rows_cut.children[0].truncated = true;
    assert_eq!(surfaces_of(&rows_cut), Some(0));
}

#[test]
fn a_frame_is_present_while_the_application_still_lists_it() {
    let frame = AtspiElement::new(":1.2", "/frame", 7);
    let listed = vec![
        (":1.2".to_owned(), "/other".to_owned()),
        (":1.2".to_owned(), "/frame".to_owned()),
    ];
    assert!(frame_listed(&listed, &frame));
    assert!(!frame_listed(&listed[..1], &frame));
    assert!(!frame_listed(
        &[(":1.3".to_owned(), "/frame".to_owned())],
        &frame
    ));
}

#[test]
fn only_a_combo_box_or_a_popup_or_expanded_element_is_searched_for_popups() {
    let none = words(&[]);
    let popup = words(&[HAS_POPUP]);
    let expanded = words(&[EXPANDED]);
    assert!(may_own_popup(Some(COMBO_BOX), &StateBits::new(&none)));
    assert!(may_own_popup(Some(43), &StateBits::new(&popup)));
    assert!(may_own_popup(Some(43), &StateBits::new(&expanded)));
    assert!(!may_own_popup(Some(43), &StateBits::new(&none)));
    assert!(!may_own_popup(None, &StateBits::new(&none)));
}
