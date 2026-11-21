use super::Level;

fn level(children: Vec<&'static str>, positions: Option<Vec<usize>>) -> Level<&'static str> {
    Level {
        children,
        children_unread: false,
        empty_means_unread: false,
        positions,
    }
}

#[test]
fn a_complete_level_gives_the_child_at_its_place_in_the_list() {
    assert_eq!(level(vec!["a", "b", "c"], None).child_at(1), Some("b"));
    assert_eq!(level(vec!["a", "b", "c"], None).child_at(3), None);
}

#[test]
fn a_partial_level_gives_the_child_that_has_the_index_in_the_parent() {
    let partial = || level(vec!["x", "y"], Some(vec![40, 57]));
    assert_eq!(partial().child_at(57), Some("y"));
    assert_eq!(partial().child_at(40), Some("x"));
    assert_eq!(partial().child_at(1), None);
    assert_eq!(partial().child_at(41), None);
}
