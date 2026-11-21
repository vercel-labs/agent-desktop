use super::{
    AncestorFacts, MAX_CLIMB, container_item_index, is_click_candidate, walk_has_container_item,
    with_container_click,
};

fn facts(selection_item: bool, container: bool) -> AncestorFacts {
    AncestorFacts {
        selection_item,
        container,
    }
}

#[test]
fn a_cell_selects_the_row_that_sits_inside_its_grid() {
    let ancestors = [facts(true, false), facts(false, true), facts(false, false)];
    assert_eq!(container_item_index(&ancestors), Some(0));
}

#[test]
fn the_nearest_selection_item_wins() {
    let ancestors = [
        facts(false, false),
        facts(true, false),
        facts(true, false),
        facts(false, true),
    ];
    assert_eq!(container_item_index(&ancestors), Some(1));
}

#[test]
fn a_selection_item_outside_any_grid_or_list_is_not_selected() {
    let ancestors = [facts(true, false), facts(false, false), facts(false, false)];
    assert_eq!(container_item_index(&ancestors), None);
}

#[test]
fn a_grid_without_a_selection_item_selects_nothing() {
    let ancestors = [facts(false, false), facts(false, true)];
    assert_eq!(container_item_index(&ancestors), None);
}

#[test]
fn a_container_below_the_item_does_not_qualify_it() {
    let ancestors = [facts(false, true), facts(true, false)];
    assert_eq!(container_item_index(&ancestors), None);
}

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn a_cell_with_no_click_of_its_own_advertises_click_through_its_row() {
    let actions = with_container_click(names(&["SetFocus"]), true, || true);
    assert_eq!(actions, names(&["SetFocus", "Click"]));
}

#[test]
fn an_element_that_already_clicks_or_is_not_a_cell_is_left_alone() {
    assert_eq!(
        with_container_click(names(&["Click"]), true, || panic!("looked up")),
        names(&["Click"])
    );
    assert_eq!(
        with_container_click(names(&["SetFocus"]), false, || panic!("looked up")),
        names(&["SetFocus"])
    );
}

#[test]
fn a_cell_outside_any_selectable_row_gets_no_click() {
    assert_eq!(
        with_container_click(Vec::new(), true, || false),
        Vec::<String>::new()
    );
}

#[test]
fn the_walk_sees_the_same_container_as_the_live_climb_within_the_same_limit() {
    let row = facts(true, false);
    let grid = facts(false, true);
    let plain = facts(false, false);
    assert!(walk_has_container_item(&[grid, row]));
    assert!(!walk_has_container_item(&[row, plain]));
    let mut far = vec![grid, row];
    far.extend(std::iter::repeat_n(plain, MAX_CLIMB - 1));
    assert!(!walk_has_container_item(&far));
    let mut near = vec![grid, row];
    near.extend(std::iter::repeat_n(plain, MAX_CLIMB - 2));
    assert!(walk_has_container_item(&near));
}

#[test]
fn only_a_grid_cell_with_an_interactive_role_may_be_offered_the_click() {
    use agent_desktop_core::LocatorField;
    let known = |role: &str| LocatorField::Known(role.to_string());
    assert!(is_click_candidate(true, &known("cell")));
    assert!(is_click_candidate(true, &known("textfield")));
    assert!(!is_click_candidate(true, &known("statictext")));
    assert!(!is_click_candidate(true, &LocatorField::Unknown));
    assert!(!is_click_candidate(false, &known("cell")));
}
