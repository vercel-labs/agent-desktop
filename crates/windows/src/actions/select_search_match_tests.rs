use super::admit_selection_match;
use agent_desktop_core::{AdapterError, ErrorCode};

fn admit(prior: Option<u8>, found: &mut Option<u8>, candidate: u8) -> Result<(), AdapterError> {
    admit_selection_match(prior.as_ref(), found, &candidate, |left, right| {
        left == right
    })
}

#[test]
fn a_match_that_is_not_the_earlier_element_is_ambiguous_even_as_the_first_of_its_scan() {
    let mut found = None;
    let error = admit(Some(1), &mut found, 2).expect_err("a second item");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(found, None);
}

#[test]
fn the_earlier_element_seen_again_is_the_match_of_this_scan() {
    let mut found = None;
    admit(Some(1), &mut found, 1).expect("the same element");
    assert_eq!(found, Some(1));
}

#[test]
fn a_second_item_in_one_scan_is_ambiguous() {
    let mut found = None;
    admit(None, &mut found, 1).expect("the first match");
    admit(None, &mut found, 1).expect("the same element again");
    let error = admit(None, &mut found, 2).expect_err("a second item");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(found, Some(1));
}
