use crate::tree::automation::UiaFailure;
use crate::tree::properties::ElementProperties;
use crate::tree::walk::walker::{NodeKey, TreeSource, WalkBudget, WalkOutcome, walk_from_root};
use crate::tree::walk::walker_fake::{EXHAUSTION, FakeTree, budget, window};
use agent_desktop_core::{Deadline, LocatorEvidence, ObservationRoot};
use std::cell::RefCell;
use std::collections::HashSet;

/// A fake that can also hand over a node's children in one read, and records
/// which nodes were read that way and which were walked one by one.
struct BatchingTree {
    tree: FakeTree,
    batched: HashSet<i32>,
    asked_at_once: RefCell<Vec<i32>>,
    walked: RefCell<Vec<i32>>,
}

impl BatchingTree {
    fn new(tree: FakeTree, batched: &[i32]) -> Self {
        Self {
            tree,
            batched: batched.iter().copied().collect(),
            asked_at_once: RefCell::new(Vec::new()),
            walked: RefCell::new(Vec::new()),
        }
    }

    fn walk(&self, budget: WalkBudget) -> WalkOutcome {
        let window = window();
        walk_from_root(self, &1, &ObservationRoot::Window(&window), budget)
            .expect("the walk assembles an observation")
    }
}

impl TreeSource for BatchingTree {
    type Node = i32;

    fn first_child(&self, node: &i32) -> Result<i32, UiaFailure> {
        self.walked.borrow_mut().push(*node);
        self.tree.first_child(node)
    }

    fn next_sibling(&self, node: &i32) -> Result<i32, UiaFailure> {
        self.tree.next_sibling(node)
    }

    fn children_at_once(
        &self,
        node: &i32,
        _properties: &ElementProperties,
        _deadline: Deadline,
    ) -> Option<Vec<i32>> {
        self.asked_at_once.borrow_mut().push(*node);
        if !self.batched.contains(node) {
            return None;
        }
        let mut children = Vec::new();
        let mut step = self.tree.first_child(node);
        while let Ok(child) = step {
            step = self.tree.next_sibling(&child);
            children.push(child);
        }
        assert_eq!(step, Err(EXHAUSTION), "the fake's lists end cleanly");
        Some(children)
    }

    fn identity(&self, node: &i32) -> NodeKey {
        self.tree.identity(node)
    }

    fn same_element(&self, left: &i32, right: &i32) -> bool {
        self.tree.same_element(left, right)
    }

    fn evidence(&self, node: &i32) -> (ElementProperties, LocatorEvidence, u64) {
        self.tree.evidence(node)
    }

    fn is_web_wrapper(&self, node: &i32, properties: &ElementProperties) -> bool {
        self.tree.is_web_wrapper(node, properties)
    }
}

fn node_count(outcome: WalkOutcome) -> usize {
    fn count(node: &agent_desktop_core::AccessibilityNode) -> usize {
        1 + node.children.iter().map(count).sum::<usize>()
    }
    count(
        &outcome
            .tree
            .into_accessibility_tree()
            .expect("core accepts the observation"),
    )
}

#[test]
fn children_read_at_once_are_not_walked_again_one_by_one() {
    let source = BatchingTree::new(FakeTree::default().with_children(1, &[2, 3, 4]), &[1]);

    let outcome = source.walk(budget(10));

    assert!(outcome.tree.is_complete());
    assert!(
        !source.walked.borrow().contains(&1),
        "the node read at once was walked a second time"
    );
    assert_eq!(node_count(outcome), 4);
}

#[test]
fn a_node_the_source_will_not_batch_is_walked_one_by_one() {
    let source = BatchingTree::new(FakeTree::default().with_children(1, &[2, 3]), &[]);

    let outcome = source.walk(budget(10));

    assert!(outcome.tree.is_complete());
    assert!(source.walked.borrow().contains(&1));
    assert_eq!(node_count(outcome), 3);
}

#[test]
fn a_node_read_only_to_count_its_children_is_never_read_at_once() {
    let source = BatchingTree::new(
        FakeTree::default()
            .with_chain(&[1, 2])
            .with_children(2, &[3, 4, 5]),
        &[1, 2],
    );

    source.walk(budget(1));

    assert!(
        !source.asked_at_once.borrow().contains(&2),
        "a boundary node's count would escape its own short deadline"
    );
    assert!(source.walked.borrow().contains(&2));
}

#[test]
fn a_list_read_at_once_is_cut_at_the_sibling_cap_like_the_walk_one_by_one() {
    let children: Vec<i32> = (2..=11).collect();
    let tree = || FakeTree::default().with_children(1, &children);
    let capped = || budget(10).with_max_siblings(4);

    let at_once = BatchingTree::new(tree(), &[1]).walk(capped());
    let one_by_one = BatchingTree::new(tree(), &[]).walk(capped());

    assert!(!at_once.tree.is_complete());
    assert_eq!(at_once.stats.traversal.limits.child_hits, 1);
    assert_eq!(
        at_once.stats.traversal.limits.child_hits,
        one_by_one.stats.traversal.limits.child_hits
    );
    assert_eq!(at_once.stats.traversal.nodes_visited, 5);
    assert_eq!(
        at_once.stats.traversal.nodes_visited,
        one_by_one.stats.traversal.nodes_visited
    );
}
