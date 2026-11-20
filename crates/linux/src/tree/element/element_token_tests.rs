use agent_desktop_core::{LocatorEvidence, LocatorField};

use super::{decode, encode};
use crate::tree::AtspiElement;
use crate::tree::element::ancestry::{child, of_root};
use crate::tree::live::test_support::evidence;

const GUID: &str = "0123456789abcdef";

fn frame() -> AtspiElement {
    let mut frame = AtspiElement::new(":1.7", "/org/a11y/atspi/accessible/10", 42);
    frame.set_ancestry(of_root(&frame));
    frame
}

fn below(parent: &AtspiElement, bus: &str, path: &str) -> AtspiElement {
    let mut element = AtspiElement::new(bus, path, 42);
    element.set_ancestry(child(parent, None));
    element
}

fn button(name: &str) -> LocatorEvidence {
    evidence("button", Some(name), None, None)
}

#[test]
fn a_frame_and_a_deep_element_round_trip() {
    let frame = frame();
    let pane = below(&frame, ":1.7", "/org/a11y/atspi/accessible/11");
    let list = below(&pane, ":1.7", "/org/a11y/atspi/accessible/12");
    let row = below(&list, ":1.7", "/org/a11y/atspi/accessible/130");
    let token = encode(GUID, &row, &button("Save")).unwrap();
    assert!(token.starts_with("atspi1:0123456789abcdef|42|:1.7|"));
    assert!(token.ends_with("|/org/a11y/atspi/accessible/|10,11,12,130"));
    let decoded = decode(&token).unwrap();
    assert_eq!((decoded.guid.as_str(), decoded.pid), (GUID, 42));
    assert_eq!(decoded.frame(), frame.path());
    assert!(decoded.names(&row));
    assert!(!decoded.names(&list));
}

#[test]
fn the_token_changes_with_identity_but_not_with_value_or_bounds() {
    let row = below(&frame(), ":1.7", "/org/a11y/atspi/accessible/11");
    let saved = encode(GUID, &row, &button("Save"));
    assert_ne!(saved, encode(GUID, &row, &button("Open")));
    let mut moved = button("Save");
    moved.value = LocatorField::Known("3".to_owned());
    moved.ref_evidence.bounds = LocatorField::Known(agent_desktop_core::Rect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    });
    assert_eq!(saved, encode(GUID, &row, &moved));
    let mut first = button("a");
    first.description = LocatorField::Known("b\u{1}\u{1f}c".to_owned());
    let mut shifted = button("a\u{1}\u{1f}b");
    shifted.description = LocatorField::Known("c".to_owned());
    assert_ne!(encode(GUID, &row, &first), encode(GUID, &row, &shifted));
    let mut unread = button("Save");
    unread.name = LocatorField::Unknown;
    assert_eq!(encode(GUID, &row, &unread), None);
}

#[test]
fn a_chain_that_leaves_the_frame_bus_has_no_token() {
    let frame = frame();
    let socket = below(&frame, ":1.7", "/org/a11y/atspi/accessible/11");
    let plug = below(&socket, ":1.9", "/org/a11y/atspi/accessible/2");
    assert_eq!(encode(GUID, &plug, &button("x")), None);
}

#[test]
fn an_element_not_walked_from_its_frame_has_no_token() {
    let loose = AtspiElement::new(":1.7", "/org/a11y/atspi/accessible/5", 42);
    assert_eq!(encode(GUID, &loose, &button("x")), None);
    assert_eq!(encode("", &frame(), &button("x")), None);
}

#[test]
fn other_retained_values_are_not_linux_tokens() {
    assert_eq!(decode("axtok1:00ff"), None);
    assert_eq!(decode("linux-menu:v1:a:0:3:abc:def"), None);
    assert_eq!(decode(":1.7/org/a11y/atspi/accessible/5"), None);
    assert_eq!(decode("atspi1:g|x|:1.7|0123456789abcdef|/a/|1"), None);
}
