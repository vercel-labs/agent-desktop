use super::*;

const TREE: [(u32, u32); 6] = [(10, 1), (11, 10), (12, 11), (13, 10), (20, 1), (21, 20)];

fn starts(pid: u32) -> Option<u64> {
    Some(match pid {
        10 => 1_000,
        11 => 9_000,
        12 => 9_500,
        13 => 2_000,
        _ => 9_900,
    })
}

#[test]
fn the_children_and_grandchildren_started_after_the_capture_are_listed() {
    assert_eq!(spawned_descendants_of(10, 8_500, &TREE, starts), [11, 12]);
}

#[test]
fn an_older_child_is_walked_through_but_not_listed() {
    let tree = [(13, 10), (14, 13)];
    let started = |pid| Some(if pid == 14 { 9_000 } else { 100 });
    assert_eq!(spawned_descendants_of(10, 8_500, &tree, started), [14]);
}

#[test]
fn another_tree_and_a_root_with_no_children_list_nothing() {
    assert!(spawned_descendants_of(20, 99_999, &TREE, starts).is_empty());
    assert!(spawned_descendants_of(99, 0, &TREE, starts).is_empty());
}

#[test]
fn a_parent_cycle_ends() {
    let tree = [(11, 10), (10, 11)];
    assert_eq!(spawned_descendants_of(10, 0, &tree, |_| Some(5_000)), [11]);
}

#[test]
fn a_descendant_whose_start_cannot_be_read_is_not_listed() {
    assert!(spawned_descendants_of(10, 0, &TREE, |_| None).is_empty());
}
