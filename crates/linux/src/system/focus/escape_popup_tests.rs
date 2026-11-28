use agent_desktop_core::DeliveryOutcome;

use super::{close_verbs, deepest_open_owner, is_open_owner, popup_open, popup_outcome};
use crate::actions::action_call::resolve_verb;
use crate::actions::write::mutation::WriteReply;
use crate::tree::properties::toolkit_trust::ToolkitKind;
use crate::tree::test_support::helpers::{read_of, words};
use crate::tree::vocabulary::role_numbers::{
    COMBO_BOX, DOCUMENT_WEB, FRAME, MENU, PUSH_BUTTON, PUSH_BUTTON_MENU, TABLE_CELL, TOGGLE_BUTTON,
};
use crate::tree::vocabulary::state_bits::{EXPANDED, HAS_POPUP, SHOWING};
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

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
#[test]
fn only_a_native_owner_with_a_showing_popup_is_an_open_owner() {
    let shown = || vec![node(MENU, &[SHOWING], Vec::new())];
    let hidden = || vec![node(MENU, &[], Vec::new())];
    assert!(is_open_owner(&node(COMBO_BOX, &[], shown())));
    assert!(!is_open_owner(&node(COMBO_BOX, &[], hidden())));
    assert!(!is_open_owner(&node(COMBO_BOX, &[EXPANDED], Vec::new())));
    assert!(is_open_owner(&node(PUSH_BUTTON, &[HAS_POPUP], shown())));
    assert!(!is_open_owner(&node(
        PUSH_BUTTON,
        &[HAS_POPUP, EXPANDED],
        Vec::new()
    )));
    assert!(!is_open_owner(&node(PUSH_BUTTON, &[EXPANDED], Vec::new())));
    assert!(!is_open_owner(&node(TABLE_CELL, &[EXPANDED], shown())));
}

#[test]
fn web_document_controls_never_qualify_for_background_escape() {
    let shown = || vec![node(MENU, &[SHOWING], Vec::new())];
    let mut web_button = node(PUSH_BUTTON, &[HAS_POPUP, EXPANDED], shown());
    web_button.in_web_document = true;
    assert!(!is_open_owner(&web_button));

    let mut web_combo = node(COMBO_BOX, &[EXPANDED], shown());
    web_combo.in_web_document = true;
    assert!(!is_open_owner(&web_combo));

    let frame = node(
        FRAME,
        &[SHOWING],
        vec![node(
            DOCUMENT_WEB,
            &[SHOWING],
            vec![node(PUSH_BUTTON, &[HAS_POPUP, EXPANDED], shown())],
        )],
    );
    let mut found = None;
    deepest_open_owner(&frame, 0, &mut found);
    assert!(found.is_none());
}

#[test]
fn background_escape_never_invokes_a_buttons_primary_action() {
    let primary = names(&["click", "press", "activate", "open", "clickAncestor"]);
    for role in [PUSH_BUTTON, PUSH_BUTTON_MENU, TOGGLE_BUTTON] {
        for toolkit in [
            ToolkitKind::Gtk3,
            ToolkitKind::Gtk4,
            ToolkitKind::Qt,
            ToolkitKind::Chromium,
            ToolkitKind::Other,
        ] {
            assert_eq!(
                resolve_verb(&primary, close_verbs(Some(role), toolkit)),
                None
            );
        }
        assert_eq!(
            resolve_verb(
                &names(&["click", "collapse"]),
                close_verbs(Some(role), ToolkitKind::Gtk3)
            ),
            Some(1)
        );
    }

    for toolkit in [
        ToolkitKind::Gtk4,
        ToolkitKind::Qt,
        ToolkitKind::Chromium,
        ToolkitKind::Other,
    ] {
        assert_eq!(
            resolve_verb(&primary, close_verbs(Some(COMBO_BOX), toolkit)),
            None
        );
    }
    assert_eq!(
        resolve_verb(
            &names(&["Press"]),
            close_verbs(Some(COMBO_BOX), ToolkitKind::Gtk3)
        ),
        Some(0)
    );
    assert_eq!(
        resolve_verb(
            &names(&["click", "activate"]),
            close_verbs(Some(COMBO_BOX), ToolkitKind::Gtk3)
        ),
        None
    );
}

#[test]
fn a_rejected_owner_action_is_never_a_delivery() {
    assert_eq!(
        popup_outcome(WriteReply::Absent, || Some(true)),
        DeliveryOutcome::NotDelivered
    );
    assert_eq!(
        popup_outcome(WriteReply::Done, || Some(true)),
        DeliveryOutcome::observed(Some(true))
    );
}

#[test]
fn a_popup_that_could_not_be_read_after_the_dismissal_is_not_closed() {
    let menu = |states: &[u32]| node(MENU, states, Vec::new());
    assert_eq!(
        popup_open(&node(COMBO_BOX, &[], vec![menu(&[SHOWING])])),
        Some(true)
    );
    assert_eq!(
        popup_open(&node(COMBO_BOX, &[], vec![menu(&[])])),
        Some(false)
    );

    let mut owner_unread = node(COMBO_BOX, &[], vec![menu(&[])]);
    owner_unread.payload.node.traits.states = Field::Failed("gone".to_owned());
    assert_eq!(popup_open(&owner_unread), None);

    let mut menu_unread = menu(&[]);
    menu_unread.payload.node.traits.states = Field::Failed("gone".to_owned());
    assert_eq!(popup_open(&node(COMBO_BOX, &[], vec![menu_unread])), None);

    let mut cut = node(COMBO_BOX, &[], vec![menu(&[])]);
    cut.truncated = true;
    assert_eq!(popup_open(&cut), None);
}
