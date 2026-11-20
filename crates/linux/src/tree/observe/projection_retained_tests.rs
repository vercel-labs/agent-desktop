use crate::tree::AtspiElement;
use crate::tree::element::ancestry::{child, of_root};
use crate::tree::live::test_support::read_of;
use crate::tree::vocabulary::role_numbers::PUSH_BUTTON;
use crate::tree::{ObservedRead, WalkLimits};

use super::project_node;

fn under_frame(name: &str) -> ObservedRead {
    let mut frame = AtspiElement::new(":1.1", "/frame", 7);
    frame.set_ancestry(of_root(&frame));
    let mut read = read_of(PUSH_BUTTON, name, &[]);
    read.node.element = AtspiElement::new(":1.1", "/frame/button", 7);
    read.node.element.set_ancestry(child(&frame, None));
    read
}

#[test]
fn every_node_walked_from_its_frame_carries_its_element_token_named_or_not() {
    for name in ["Save", ""] {
        let evidence = project_node(&under_frame(name), false, &WalkLimits::default(), "g");
        let token = evidence.identifiers.retained_object().expect("a token");
        assert!(token.starts_with("token:atspi1:g|7|:1.1|"));
        assert!(token.ends_with("|/|frame,frame/button"));
    }
}
