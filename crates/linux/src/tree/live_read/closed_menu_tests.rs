use agent_desktop_core::{AdapterError, Deadline, LiveElement};

use super::reach_closed_menu_item;
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::element::ancestry;
use crate::tree::live_read::NodeReader;
use crate::tree::live_read::live_project::live_element;
use crate::tree::observe::node_evidence::{NodeEvidence, evidence_for};
use crate::tree::placement::Placement;
use crate::tree::properties::liveness::Liveness;
use crate::tree::test_support::helpers::{read_of, words};
use crate::tree::vocabulary::role_numbers::{MENU_BAR, MENU_ITEM, POPUP_MENU};
use crate::tree::vocabulary::state_bits::{SHOWING, VISIBLE};
use crate::tree::{AtspiElement, Field, ObservedRead};

struct EntryReader {
    entry_showing: bool,
}

impl NodeReader for EntryReader {
    fn read(&mut self, element: &AtspiElement) -> Result<NodeEvidence, AdapterError> {
        crate::tree::observe::node_evidence::require_ancestry(element)?;
        let bits: &[u32] = if self.entry_showing {
            &[SHOWING, VISIBLE]
        } else {
            &[]
        };
        let mut read = read_of(MENU_ITEM, "Settings", &["ShowMenu"]);
        read.node.element = element.clone();
        read.node.traits.states = Field::Present(words(bits));
        let evidence = evidence_for(&read, element, "");
        Ok(NodeEvidence {
            read,
            evidence,
            liveness: Liveness::Alive,
        })
    }

    fn stopped_app(&mut self, _pid: u32) -> Option<String> {
        None
    }

    fn placement(&mut self, _element: &AtspiElement, _deadline: Deadline) -> Placement {
        Placement::default()
    }

    fn global_position(&mut self, _element: &AtspiElement, _deadline: Deadline) -> bool {
        false
    }
}

fn closed_item() -> AtspiElement {
    let mut bar = AtspiElement::new(":1.1", "/bar", 1);
    bar.set_ancestry(ancestry::of_root(&AtspiElement::new(":1.1", "/frame", 1)));
    let mut entry = AtspiElement::new(":1.1", "/entry", 1);
    entry.set_ancestry(ancestry::child(&bar, Some(MENU_BAR)));
    let mut menu = AtspiElement::new(":1.1", "/menu", 1);
    menu.set_ancestry(ancestry::child(&entry, Some(MENU_ITEM)));
    let mut item = AtspiElement::new(":1.1", "/item", 1);
    item.set_ancestry(ancestry::child(&menu, Some(POPUP_MENU)));
    item
}

fn item_read(bits: &[u32]) -> ObservedRead {
    let mut read = read_of(MENU_ITEM, "Show Path in Titlebar", &["Press"]);
    read.node.traits.states = Field::Present(words(bits));
    read
}

fn project(entry_showing: bool, item_bits: &[u32], toolkit: &str) -> LiveElement {
    let element = closed_item();
    let read = item_read(item_bits);
    let evidence = evidence_for(&read, &element, "");
    let mut live = live_element(&read, &evidence, &Placement::default(), true).unwrap();
    let toolkit = ToolkitId {
        name: Some(toolkit.to_owned()),
        version: Some("5.15.13".to_owned()),
    };
    let mut reader = EntryReader { entry_showing };
    reach_closed_menu_item(&mut reader, &element, &read, &toolkit, &mut live);
    live
}

#[test]
fn only_a_closed_qt_item_under_a_showing_menu_bar_entry_reads_reachable() {
    let rows: [(bool, &[u32], &str, Option<bool>); 4] = [
        (true, &[], "Qt", Some(false)),
        (false, &[], "Qt", None),
        (true, &[], "GTK", None),
        (true, &[SHOWING, VISIBLE], "Qt", None),
    ];
    for (entry_showing, item_bits, toolkit, hidden) in rows {
        let live = project(entry_showing, item_bits, toolkit);
        assert_eq!(
            live.state.hidden, hidden,
            "{entry_showing} {item_bits:?} {toolkit}"
        );
        assert_eq!(live.state.offscreen, None);
    }
    let closed = project(true, &[], "Qt");
    assert!(closed.state.states.iter().any(|token| token == "hidden"));
}
