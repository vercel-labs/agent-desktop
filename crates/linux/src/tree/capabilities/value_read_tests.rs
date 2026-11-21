use super::{
    MAX_TEXT_CHARACTERS, ValueSource, number_text, reaches_text_cap, truncate_to_boundary,
    value_source,
};
use crate::tree::test_support::helpers::words;
use crate::tree::vocabulary::role_numbers::{
    COMBO_BOX, ENTRY, PANEL, PASSWORD_TEXT, PROGRESS_BAR, PUSH_BUTTON, SLIDER, TEXT,
};
use crate::tree::vocabulary::state_bits::EDITABLE;
use crate::tree::{AtspiElement, Field, RawNode};

fn node(role: u32, interfaces: &[&str], state_bits: &[u32]) -> RawNode {
    let mut node = RawNode::unreadable(AtspiElement::new(":1.1", "/p", 1), "unused");
    node.traits.role = Field::Present(role);
    node.traits.states = Field::Present(words(state_bits));
    node.traits.interfaces = Field::Present(
        interfaces
            .iter()
            .map(|name| format!("org.a11y.atspi.{name}"))
            .collect(),
    );
    node
}

#[test]
fn a_slider_and_a_progress_bar_with_the_value_interface_read_a_number() {
    assert_eq!(
        value_source(&node(SLIDER, &["Value"], &[])),
        ValueSource::Numeric
    );
    assert_eq!(
        value_source(&node(PROGRESS_BAR, &["Value"], &[])),
        ValueSource::Numeric
    );
}

#[test]
fn a_slider_that_does_not_name_the_value_interface_is_never_read() {
    assert_eq!(value_source(&node(SLIDER, &[], &[])), ValueSource::None);
}

#[test]
fn an_entry_and_an_editable_text_node_read_text() {
    assert_eq!(
        value_source(&node(ENTRY, &["Text"], &[])),
        ValueSource::Text
    );
    assert_eq!(
        value_source(&node(TEXT, &["Text"], &[EDITABLE])),
        ValueSource::Text
    );
    assert_eq!(
        value_source(&node(COMBO_BOX, &["Text"], &[])),
        ValueSource::Text
    );
}

#[test]
fn read_only_text_is_content_and_carries_no_value() {
    assert_eq!(value_source(&node(TEXT, &["Text"], &[])), ValueSource::None);
}

#[test]
fn a_password_field_has_no_value_source() {
    assert_eq!(
        value_source(&node(PASSWORD_TEXT, &["Text"], &[EDITABLE])),
        ValueSource::None
    );
}

#[test]
fn a_role_without_a_value_is_never_read() {
    assert_eq!(
        value_source(&node(PUSH_BUTTON, &["Value", "Text"], &[])),
        ValueSource::None
    );
    assert_eq!(
        value_source(&node(PANEL, &["Value", "Text"], &[])),
        ValueSource::None
    );
}

#[test]
fn a_node_whose_role_read_failed_has_an_unknown_value_source() {
    let unreadable = RawNode::unreadable(AtspiElement::new(":1.1", "/p", 1), "failed");
    assert_eq!(value_source(&unreadable), ValueSource::Unknown);
}

#[test]
fn unread_value_prerequisites_cannot_verify_a_click_effect() {
    use crate::tree::capabilities::action_read::NodeView;
    use crate::tree::observe::node_evidence::{NodeEvidence, evidence_for};
    use crate::tree::properties::liveness::Liveness;
    use crate::tree::test_support::helpers::read_of;
    use zbus::connection::Builder;

    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        Builder::async_io_unix_stream(far)
            .server(zbus::Guid::generate())
            .unwrap()
            .p2p()
            .build(),
    ));
    let client = client.unwrap();
    let _server = server.unwrap();
    let view = |read: crate::tree::ObservedRead| {
        NodeView::new(NodeEvidence {
            evidence: evidence_for(&read, &read.node.element, ""),
            read,
            liveness: Liveness::Alive,
        })
    };
    let mut before = read_of(ENTRY, "Entry", &["click"]);
    before.value.value = Field::Present("unchanged".to_owned());
    let previous = view(before.clone()).observation();
    for prerequisite in ["interfaces", "role", "states"] {
        let mut after = before.clone();
        if prerequisite == "role" {
            after.node.traits.role = Field::Failed("unreadable".to_owned());
        } else if prerequisite == "states" {
            after.node.traits.role = Field::Present(TEXT);
            after.node.traits.interfaces = Field::Present(vec!["org.a11y.atspi.Text".to_owned()]);
            after.node.traits.states = Field::Failed("unreadable".to_owned());
        } else {
            after.node.traits.interfaces = Field::Failed("unreadable".to_owned());
        }
        after.value = zbus::block_on(super::read_value(&client, &after.node, 64)).unwrap();
        assert!(after.value.value.is_failed(), "{prerequisite}");
        assert_eq!(
            view(after).observation().changed_since(&previous),
            None,
            "{prerequisite}"
        );
    }
}

#[test]
fn value_connection_loss_is_not_field_uncertainty() {
    let connection = crate::tree::test_support::helpers::closed_connection();
    for node in [node(ENTRY, &["Text"], &[]), node(SLIDER, &["Value"], &[])] {
        let error = zbus::block_on(super::read_value(&connection, &node, 64))
            .expect_err("a lost bus must end observation");
        assert_eq!(error.code, agent_desktop_core::ErrorCode::Internal);
    }
}

#[test]
fn text_longer_than_the_budget_is_cut_on_a_character_boundary() {
    let (kept, cut) = truncate_to_boundary("aé€😀".repeat(3), 10);
    assert!(cut);
    assert!(kept.len() <= 10);
    assert!(kept.starts_with("aé€"));
}

#[test]
fn text_within_the_budget_is_untouched() {
    let (kept, cut) = truncate_to_boundary("héllo".to_owned(), 64);
    assert_eq!((kept.as_str(), cut), ("héllo", false));
}

#[test]
fn a_budget_that_lands_inside_a_multibyte_character_backs_up_to_its_start() {
    let (kept, cut) = truncate_to_boundary("€€".to_owned(), 4);
    assert_eq!((kept.as_str(), cut), ("€", true));
    let (empty, cut) = truncate_to_boundary("€".to_owned(), 2);
    assert_eq!((empty.as_str(), cut), ("", true));
}

#[test]
fn numbers_print_without_a_trailing_fraction_and_non_finite_numbers_are_refused() {
    assert_eq!(number_text(50.0).as_deref(), Some("50"));
    assert_eq!(number_text(0.25).as_deref(), Some("0.25"));
    assert_eq!(number_text(f64::NAN), None);
    assert_eq!(number_text(f64::INFINITY), None);
}

#[test]
fn a_table_cell_with_the_text_interface_reads_its_content_editable_or_not() {
    use crate::tree::vocabulary::role_numbers::TABLE_CELL;
    assert_eq!(
        value_source(&node(TABLE_CELL, &["Text"], &[])),
        ValueSource::CellText
    );
    assert_eq!(
        value_source(&node(TABLE_CELL, &["Text", "Value"], &[EDITABLE])),
        ValueSource::CellText
    );
    assert_eq!(
        value_source(&node(TABLE_CELL, &["Component"], &[])),
        ValueSource::None
    );
}

mod cell_text_on_a_bus {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use zbus::connection::Builder;

    use super::node;
    use crate::tree::Field;
    use crate::tree::capabilities::value_read::read_value;
    use crate::tree::vocabulary::role_numbers::TABLE_CELL;

    struct Text {
        content: &'static str,
        calls: Arc<AtomicUsize>,
    }

    #[zbus::interface(name = "org.a11y.atspi.Text")]
    impl Text {
        fn get_text(&self, _start: i32, end: i32) -> String {
            if end != super::super::MAX_TEXT_CHARACTERS {
                return format!("unbounded end {end}");
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.content.to_owned()
        }

        #[zbus(property)]
        fn character_count(&self) -> i32 {
            self.calls.fetch_add(100, Ordering::SeqCst);
            0
        }
    }

    fn read(content: &'static str) -> (Field<String>, usize) {
        let calls = Arc::new(AtomicUsize::new(0));
        let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
        let (client, server) = zbus::block_on(futures_lite::future::zip(
            Builder::async_io_unix_stream(near).p2p().build(),
            Builder::async_io_unix_stream(far)
                .server(zbus::Guid::generate())
                .unwrap()
                .p2p()
                .serve_at(
                    "/p",
                    Text {
                        content,
                        calls: calls.clone(),
                    },
                )
                .unwrap()
                .build(),
        ));
        let _server = server.unwrap();
        let cell = node(TABLE_CELL, &["Text"], &[]);
        let read = zbus::block_on(read_value(&client.unwrap(), &cell, 1024)).unwrap();
        (read.value, calls.load(Ordering::SeqCst))
    }

    #[test]
    fn a_cell_is_read_with_one_text_call_and_no_length_call() {
        let (value, calls) = read("Flux");
        assert_eq!(value.present().map(String::as_str), Some("Flux"));
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_blank_cell_has_no_value() {
        let (value, calls) = read("");
        assert!(matches!(value, Field::Absent));
        assert_eq!(calls, 1);
    }
}

#[test]
fn a_cell_read_that_fills_the_character_cap_may_go_on_past_it() {
    let cap = usize::try_from(MAX_TEXT_CHARACTERS).unwrap();
    assert!(reaches_text_cap(&"é".repeat(cap)));
    assert!(!reaches_text_cap(&"é".repeat(cap - 1)));
    assert!(!reaches_text_cap(""));
}
