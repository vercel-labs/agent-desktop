use super::*;

fn chain(element: u32, container: Option<u32>, open: &[u32]) -> bool {
    reaches_open_window(
        &element,
        container,
        |node| open.contains(node),
        |node| Ok((*node > 1).then(|| node - 1)),
    )
    .unwrap()
}

#[test]
fn an_open_window_element_that_reports_no_window_container_is_live() {
    assert!(chain(5, None, &[5]));
}

#[test]
fn a_closed_window_element_that_reports_no_window_container_is_not_live() {
    assert!(!chain(5, None, &[9]));
}

#[test]
fn a_child_is_live_when_its_container_or_an_ancestor_is_open() {
    assert!(chain(7, Some(5), &[5]));
    assert!(chain(7, Some(5), &[3]));
    assert!(!chain(7, Some(5), &[6]));
}
