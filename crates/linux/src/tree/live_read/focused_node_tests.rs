use std::collections::BTreeMap;

use super::*;

#[derive(Default)]
struct FakeTree {
    states: BTreeMap<u32, Vec<u32>>,
    children: BTreeMap<u32, Vec<u32>>,
    collection: BTreeMap<u32, Option<u32>>,
    read: Vec<u32>,
}

fn words(bits: &[u32]) -> Vec<u32> {
    let mut words = vec![0, 0];
    for bit in bits {
        words[(*bit / 32) as usize] |= 1 << (bit % 32);
    }
    words
}

impl FakeTree {
    fn node(mut self, id: u32, bits: &[u32], children: &[u32]) -> Self {
        self.states.insert(id, words(bits));
        self.children.insert(id, children.to_vec());
        self
    }
}

impl FocusSource for FakeTree {
    type Node = u32;

    fn states(&mut self, node: &u32) -> Result<Option<Vec<u32>>, AdapterError> {
        self.read.push(*node);
        Ok(self.states.get(node).cloned())
    }

    fn children(&mut self, node: &u32) -> Result<Vec<u32>, AdapterError> {
        Ok(self.children.get(node).cloned().unwrap_or_default())
    }

    fn collection_focus(&mut self, frame: &u32) -> Result<Option<Option<u32>>, AdapterError> {
        Ok(self.collection.get(frame).copied())
    }
}

#[test]
fn collection_answers_for_a_frame_that_implements_it() {
    let mut tree = FakeTree::default()
        .node(1, &[ACTIVE], &[2])
        .node(2, &[SHOWING, FOCUSED], &[]);
    tree.collection.insert(1, Some(9));
    assert_eq!(focus_in(&mut tree, &1).expect("search"), Some(9));
    tree.collection.insert(1, None);
    assert_eq!(focus_in(&mut tree, &1).expect("search"), None);
    assert!(tree.read.is_empty(), "no walk after Collection answered");
}

#[test]
fn a_walk_finds_focus_below_showing_nodes_and_skips_hidden_subtrees() {
    let mut tree = FakeTree::default()
        .node(1, &[ACTIVE], &[2, 4])
        .node(2, &[], &[3])
        .node(3, &[FOCUSED], &[])
        .node(4, &[SHOWING], &[5])
        .node(5, &[SHOWING, FOCUSED], &[]);
    assert_eq!(focus_in(&mut tree, &1).expect("search"), Some(5));
    assert!(!tree.read.contains(&3), "a hidden subtree is not read");
}

#[test]
fn only_an_active_frame_is_searched() {
    let mut tree = FakeTree::default()
        .node(0, &[], &[1, 2])
        .node(1, &[], &[3])
        .node(3, &[SHOWING, FOCUSED], &[])
        .node(2, &[ACTIVE], &[4])
        .node(4, &[SHOWING, FOCUSED], &[]);
    assert_eq!(
        focused_under(&mut tree, vec![0]).expect("search"),
        Some((2, 4))
    );
}

#[test]
fn no_active_frame_means_no_focus() {
    let mut tree = FakeTree::default()
        .node(0, &[], &[1])
        .node(1, &[], &[3])
        .node(3, &[SHOWING, FOCUSED], &[]);
    assert_eq!(focused_under(&mut tree, vec![0]).expect("search"), None);
}

#[test]
fn a_walk_stops_at_its_visit_budget() {
    let count = MAX_FOCUS_VISITS as u32 + 10;
    let mut tree = FakeTree::default().node(0, &[ACTIVE], &(1..=count).collect::<Vec<_>>());
    for id in 1..count {
        tree = tree.node(id, &[SHOWING], &[]);
    }
    tree = tree.node(count, &[SHOWING, FOCUSED], &[]);
    assert_eq!(focus_in(&mut tree, &0).expect("search"), None);
    assert_eq!(tree.read.len(), MAX_FOCUS_VISITS);
}
