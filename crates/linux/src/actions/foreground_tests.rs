use agent_desktop_core::{Action, ActionRequest, AdapterError, Deadline, HeadedRequirement};

use super::{headed_requirement, takes_foreground};
use crate::tree::Field;
use crate::tree::capabilities::action_read::NodeView;
use crate::tree::live::test_support::{read_of, view_of, words};
use crate::tree::properties::toolkit_trust::ToolkitKind::{self, Gtk3, Gtk4, Other, Qt};
use crate::tree::vocabulary::role_numbers::{COMBO_BOX, MENU_ITEM, PUSH_BUTTON, TOGGLE_BUTTON};
use crate::tree::vocabulary::state_bits::{HAS_POPUP, SHOWING};

/// A node as pinned on linuxbox: role, name, actions, whether it carries
/// HAS_POPUP and whether it offers Selection.
type Node = (u32, &'static str, &'static [&'static str], bool, bool);

fn view((role, name, actions, popup, selection): Node) -> NodeView {
    let mut read = read_of(role, name, actions);
    let bits: &[u32] = if popup {
        &[HAS_POPUP, SHOWING]
    } else {
        &[SHOWING]
    };
    read.node.traits.states = Field::Present(words(bits));
    let selection = selection.then(|| "org.a11y.atspi.Selection".to_owned());
    read.node.traits.interfaces = Field::Present(selection.into_iter().collect());
    view_of(read)
}

const DROP_DOWN: Node = (COMBO_BOX, "Degrees", &[], true, false);
const MENU_BUTTON: Node = (PUSH_BUTTON, "Main Menu", &[], true, false);
const TOGGLE: Node = (TOGGLE_BUTTON, "Main Menu", &["Click"], false, false);
const MENU_BAR_ENTRY: Node = (MENU_ITEM, "Settings", &["ShowMenu"], false, false);
const CONFIGURE: Node = (MENU_ITEM, "Configure Kate...", &["Press"], false, false);
const TEST: Node = (PUSH_BUTTON, "Test…", &["Click"], false, false);

/// One row per element measured in wave0 (C2, C3, L-W0-11 to L-W0-13).
#[test]
fn each_measured_element_is_refused_exactly_when_it_took_the_foreground() {
    let rows: [(Action, Node, ToolkitKind, Option<Node>, bool); 16] = [
        (
            Action::Select("Gradians".into()),
            DROP_DOWN,
            Gtk4,
            None,
            true,
        ),
        (Action::Expand, DROP_DOWN, Gtk4, None, true),
        (Action::Click, TOGGLE, Gtk4, Some(DROP_DOWN), true),
        (Action::Click, TOGGLE, Gtk4, Some(MENU_BUTTON), true),
        (Action::Click, TOGGLE, Gtk3, Some(MENU_BUTTON), false),
        (
            Action::Select("Speakers".into()),
            (COMBO_BOX, "Output Device", &[], false, true),
            Gtk4,
            None,
            false,
        ),
        (Action::Click, MENU_BAR_ENTRY, Qt, None, true),
        (Action::RightClick, MENU_BAR_ENTRY, Qt, None, true),
        (Action::Click, MENU_BAR_ENTRY, Gtk3, None, true),
        (
            Action::Click,
            (MENU_ITEM, "Show Path in Titlebar", &["Press"], false, false),
            Qt,
            None,
            false,
        ),
        (Action::Click, CONFIGURE, Qt, None, true),
        (Action::Click, CONFIGURE, Other, None, true),
        (Action::RightClick, CONFIGURE, Qt, None, false),
        (Action::Click, TEST, Gtk4, None, true),
        (Action::Click, TEST, Gtk3, None, false),
        (
            Action::Check,
            (
                TOGGLE_BUTTON,
                "Overamplification",
                &["Toggle"],
                false,
                false,
            ),
            Gtk4,
            None,
            false,
        ),
    ];
    for (action, element, toolkit, owner, expected) in rows {
        let owner = || Ok::<_, AdapterError>(owner.map(view));
        let takes = takes_foreground(&action, &view(element), toolkit, owner).unwrap();
        assert_eq!(takes, expected, "{action:?} on {} ({toolkit:?})", element.1);
    }
}

#[test]
fn a_field_the_read_could_not_answer_never_proves_an_action_safe() {
    let mut unnamed = read_of(PUSH_BUTTON, "Test…", &["Click"]);
    unnamed.node.properties.name = Field::Failed("unread".to_owned());
    let mut no_actions = read_of(MENU_ITEM, "Settings", &[]);
    no_actions.node.actions = Field::Failed("unread".to_owned());
    let mut no_states = read_of(COMBO_BOX, "Degrees", &[]);
    no_states.node.traits.states = Field::Failed("unread".to_owned());
    for (action, read, toolkit) in [
        (Action::Click, unnamed, Gtk4),
        (Action::Click, no_actions, Gtk3),
        (Action::Expand, no_states, Gtk4),
    ] {
        let none = || Ok::<_, AdapterError>(None);
        assert!(takes_foreground(&action, &view_of(read), toolkit, none).unwrap());
    }
}

#[test]
fn a_headless_focus_needs_the_application_and_a_headed_one_keeps_its_requirement() {
    let bus = crate::system::SharedBus::default();
    let handle = crate::tree::AtspiElement::new(":1.9", "/item", 9).into_native_handle();
    let deadline = Deadline::standard().unwrap();
    let headless = ActionRequest::headless(Action::SetFocus);
    let headed = ActionRequest::headed(Action::SetFocus);
    let of =
        |request: &ActionRequest| headed_requirement(&bus, &handle, request, deadline).unwrap();
    assert_eq!(of(&headless), HeadedRequirement::FocusedApplication);
    assert_eq!(of(&headed), headed.headed_requirement());
}
