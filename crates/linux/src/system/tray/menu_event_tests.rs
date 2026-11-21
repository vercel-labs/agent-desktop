use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_desktop_core::{Deadline, DeliveryDisposition, DeliverySemantics, ErrorCode};
use zbus::connection::Builder;
use zbus::zvariant::{Array, Dict, OwnedValue, Structure, Value};

use super::*;

fn entry() -> LayoutEntry {
    LayoutEntry {
        id: 4,
        label: "Open".into(),
        enabled: true,
        separator: false,
        submenu: false,
        checked: None,
        children: Vec::new(),
    }
}

#[test]
fn an_enabled_plain_entry_is_picked_with_a_click_event() {
    assert_eq!(validate(&entry()).ok(), Some(Pick::Clicked));
}

#[test]
fn a_disabled_entry_is_refused_before_anything_is_sent() {
    let mut disabled = entry();
    disabled.enabled = false;
    let error = validate(&disabled).expect_err("refused");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn a_separator_is_refused_even_when_it_is_enabled() {
    let mut separator = entry();
    separator.separator = true;
    let error = validate(&separator).expect_err("refused");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}

#[test]
fn a_submenu_parent_opens_its_submenu_and_is_never_clicked() {
    let mut parent = entry();
    parent.submenu = true;
    assert_eq!(validate(&parent).ok(), Some(Pick::ShowSubmenu));
}

#[test]
fn a_disabled_submenu_parent_is_refused() {
    let mut parent = entry();
    parent.submenu = true;
    parent.enabled = false;
    assert!(validate(&parent).is_err());
}

type LayoutReply = (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>));

struct Menu {
    populated: AtomicBool,
    eager: bool,
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn get_layout(
        &self,
        _parent: i32,
        _depth: i32,
        _properties: Vec<String>,
    ) -> zbus::fdo::Result<LayoutReply> {
        Ok(layout(self.populated.load(Ordering::Relaxed), self.eager))
    }

    fn about_to_show(&self, _id: i32) {
        self.populated.store(true, Ordering::Relaxed);
    }
}

fn owned(value: Value<'_>) -> OwnedValue {
    OwnedValue::try_from(value).expect("a plain value converts")
}

fn wire_entry(id: i32, label: &str, submenu: bool, children: Vec<OwnedValue>) -> OwnedValue {
    let mut props = HashMap::new();
    props.insert("label".to_owned(), owned(Value::from(label)));
    if submenu {
        props.insert("children-display".to_owned(), owned(Value::from("submenu")));
    }
    let structure = Structure::from((
        Value::I32(id),
        Value::Dict(Dict::from(props)),
        Value::Array(Array::from(children)),
    ));
    OwnedValue::try_from(Value::Structure(structure)).expect("a menu entry converts")
}

fn layout(populated: bool, eager: bool) -> LayoutReply {
    let more_children = if eager || populated {
        vec![wire_entry(6, "Kid", false, Vec::new())]
    } else {
        Vec::new()
    };
    let more = wire_entry(5, "More", true, more_children);
    (7, (0, HashMap::new(), vec![more]))
}

fn connection(eager: bool) -> (zbus::Connection, zbus::Connection) {
    let (client, server) = UnixStream::pair().unwrap();
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(client).p2p().build(),
        Builder::async_io_unix_stream(server)
            .server(zbus::Guid::generate())
            .unwrap()
            .p2p()
            .serve_at(
                "/Menu",
                Menu {
                    populated: AtomicBool::new(false),
                    eager,
                },
            )
            .unwrap()
            .build(),
    ));
    (client.unwrap(), server.unwrap())
}

fn target() -> TrayTarget {
    TrayTarget {
        owner: ":1.20".into(),
        object_path: "/Item".into(),
        menu_path: Some("/Menu".into()),
        entry_id: Some(5),
        label_path: vec!["More".into()],
    }
}

#[test]
fn a_submenu_populated_by_about_to_show_is_verified() {
    let (client, _server) = connection(false);
    let result =
        deliver(&client, &target(), Deadline::after(2_000).unwrap()).expect("the submenu opens");
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
}

#[test]
fn a_submenu_that_already_had_children_is_not_verified_by_the_call() {
    let (client, _server) = connection(true);
    let result =
        deliver(&client, &target(), Deadline::after(2_000).unwrap()).expect("the submenu opens");
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );
}
