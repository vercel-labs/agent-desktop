use super::unique_index;
use agent_desktop_core::ErrorCode;

#[test]
fn a_name_found_once_selects_its_index() {
    assert_eq!(unique_index(3, 3).expect("unique"), 3);
}

#[test]
fn a_missing_name_is_element_not_found() {
    assert_eq!(
        unique_index(-1, -1).expect_err("missing").code,
        ErrorCode::ElementNotFound
    );
}

#[test]
fn a_name_found_twice_is_ambiguous_and_selects_nothing() {
    assert_eq!(
        unique_index(1, 4).expect_err("duplicate").code,
        ErrorCode::AmbiguousTarget
    );
}
