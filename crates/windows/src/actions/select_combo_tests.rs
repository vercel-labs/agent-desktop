use super::{combo_has_strings, index_or_fall_through};
use agent_desktop_core::{AdapterError, ErrorCode};

#[test]
fn only_an_owner_drawn_combo_without_the_has_strings_style_lacks_item_names() {
    assert!(combo_has_strings(0x0003));
    assert!(combo_has_strings(0x0200 | 0x0003));
    assert!(combo_has_strings(0x0010 | 0x0200 | 0x0003));
    assert!(!combo_has_strings(0x0010 | 0x0003));
    assert!(!combo_has_strings(0x0020 | 0x0003));
}

#[test]
fn a_name_the_search_misses_falls_through_while_other_failures_stay_failures() {
    assert_eq!(index_or_fall_through(Ok(3)).unwrap(), Some(3));
    let missing = AdapterError::new(ErrorCode::ElementNotFound, "no item");
    assert_eq!(index_or_fall_through(Err(missing)).unwrap(), None);
    let ambiguous = AdapterError::ambiguous_target("two items");
    assert_eq!(
        index_or_fall_through(Err(ambiguous)).unwrap_err().code,
        ErrorCode::AmbiguousTarget
    );
    let hung = AdapterError::new(ErrorCode::AppUnresponsive, "hung");
    assert_eq!(
        index_or_fall_through(Err(hung)).unwrap_err().code,
        ErrorCode::AppUnresponsive
    );
}
