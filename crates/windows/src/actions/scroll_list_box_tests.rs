use super::top_item_or_fall_through;
use agent_desktop_core::{AdapterError, ErrorCode};
use std::cell::Cell;

#[test]
fn a_name_the_list_box_does_not_hold_falls_through_to_the_next_scroll_route() {
    let sent = Cell::new(false);
    let missing = AdapterError::new(ErrorCode::ElementNotFound, "no item");
    let result = top_item_or_fall_through(Err(missing), |_| {
        sent.set(true);
        Ok(true)
    });
    assert!(!result.expect("fall through"));
    assert!(!sent.get());
}

#[test]
fn an_ambiguous_or_hung_search_stays_an_error_and_a_found_item_is_sent() {
    let ambiguous = AdapterError::ambiguous_target("two items");
    let error = top_item_or_fall_through(Err(ambiguous), |_| Ok(true)).expect_err("ambiguous");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    let hung = AdapterError::new(ErrorCode::AppUnresponsive, "hung");
    let error = top_item_or_fall_through(Err(hung), |_| Ok(true)).expect_err("hung");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert!(top_item_or_fall_through(Ok(4), |index| Ok(index == 4)).expect("sent"));
}
