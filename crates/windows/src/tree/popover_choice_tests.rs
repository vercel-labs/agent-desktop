//! Popover selection: which open popover a capture, a surface read or a stored
//! ref lands on when the process has none, one or several.

use super::{choose_popover, handle_id};
use crate::system::window::window_enum::WindowHandle;
use agent_desktop_core::ErrorCode;
use serde_json::json;

fn handle(number: usize) -> WindowHandle {
    number as WindowHandle
}

#[test]
fn two_open_popovers_without_an_id_are_refused_with_their_ids() {
    let error = choose_popover(&[handle(17), handle(34)], None).unwrap_err();

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert!(
        error.message.contains("w-17") && error.message.contains("w-34"),
        "the refusal names the open popovers: {}",
        error.message
    );
    let details = error.details.expect("ambiguous popover carries candidates");
    assert_eq!(details["kind"], "popover_ambiguous");
    assert_eq!(details["candidates"], json!(["w-17", "w-34"]));
}

#[test]
fn a_named_popover_is_the_one_returned_even_with_two_open() {
    let chosen = choose_popover(&[handle(17), handle(34)], Some("w-34")).expect("named popover");

    assert_eq!(handle_id(chosen), "w-34");
}

#[test]
fn a_name_that_is_not_an_open_popover_is_refused() {
    let error = choose_popover(&[handle(17)], Some("w-34")).unwrap_err();

    assert_eq!(error.code, ErrorCode::WindowNotFound);
    let details = error.details.expect("a closed popover lists what is open");
    assert_eq!(details["kind"], "popover_closed");
    assert_eq!(details["window_id"], "w-34");
    assert_eq!(details["candidates"], json!(["w-17"]));
}

#[test]
fn no_open_popover_is_a_missing_window() {
    let error = choose_popover(&[], None).unwrap_err();

    assert_eq!(error.code, ErrorCode::WindowNotFound);
}

#[test]
fn one_open_popover_is_unambiguous() {
    assert_eq!(
        handle_id(choose_popover(&[handle(17)], None).expect("one popover")),
        "w-17"
    );
}
