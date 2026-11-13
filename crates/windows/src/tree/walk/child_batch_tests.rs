use super::*;
use crate::tree::automation::{automation_client, root_from_hwnd};
use crate::tree::fixture::{HostedFixture, bootstrap};
use crate::tree::properties::{read_cached, read_live};
use crate::tree::walk::cache::build_children_cache_request;
use agent_desktop_core::Deadline;

const DEPTH: usize = 6;

fn fixture_root(fixture: &HostedFixture) -> UIAElement {
    root_from_hwnd(
        fixture.handle(),
        Deadline::standard().expect("a standard deadline"),
    )
    .expect("the fixture window resolves")
}

fn batch() -> ChildBatch {
    let client = automation_client().expect("a UIA client");
    ChildBatch::new(build_children_cache_request(&client).expect("the request builds"))
}

fn deadline() -> Deadline {
    Deadline::standard().expect("a standard deadline")
}

fn control_type(id: i32) -> ElementProperties {
    ElementProperties::from_reads(vec![(
        TreeProperty::ControlType,
        PropertyOutcome::Known(PropertyValue::Number(id)),
    )])
}

fn runtime_id(element: &UIAElement) -> Vec<i32> {
    element.0.get_runtime_id().expect("a runtime id")
}

fn live_children(node: &UIAElement) -> Vec<UIAElement> {
    let walker = automation_client()
        .expect("a UIA client")
        .get_raw_view_walker()
        .expect("the raw view walker is available");
    let mut children = Vec::new();
    let mut step = walker.get_first_child(&node.0);
    while let Ok(child) = step {
        step = walker.get_next_sibling(&child);
        children.push(UIAElement::from(child));
    }
    children
}

/// Compares the batch with the raw view walker on every node down to `depth`,
/// returning how many nodes the batch read and how many of those had no
/// children.
fn compare_below(batch: &ChildBatch, node: &UIAElement, depth: usize) -> (usize, usize) {
    let live = live_children(node);
    let batched = batch
        .read(node, &control_type(50033), deadline())
        .expect("the batch reads a container's children, or that it has none");
    assert_eq!(
        batched.iter().map(runtime_id).collect::<Vec<_>>(),
        live.iter().map(runtime_id).collect::<Vec<_>>(),
        "the batch read different children, or the same children in another order, than the raw view walker"
    );
    let mut counts = (1, usize::from(batched.is_empty()));
    if depth > 0 {
        for child in &batched {
            let (read, empty) = compare_below(batch, child, depth - 1);
            counts = (counts.0 + read, counts.1 + empty);
        }
    }
    counts
}

#[test]
fn the_batch_reads_the_same_children_in_the_same_order_as_the_raw_view() {
    bootstrap();
    let fixture = HostedFixture::spawn().expect("the fixture host starts");
    let (read, empty) = compare_below(&batch(), &fixture_root(&fixture), DEPTH);
    assert!(read > 1, "the fixture exposes more than one control");
    assert!(
        empty > 0,
        "a node without children is answered by the batch itself, not left to a second read"
    );
}

#[test]
fn batched_children_carry_the_properties_the_walk_reads() {
    bootstrap();
    let fixture = HostedFixture::spawn().expect("the fixture host starts");
    let children = batch()
        .read(&fixture_root(&fixture), &control_type(50032), deadline())
        .expect("the window's children are read at once");
    assert!(!children.is_empty(), "the fixture exposes child controls");
    for child in &children {
        let (cached, _) = read_cached(child);
        let (live, _) = read_live(child);
        for property in TreeProperty::WALK_SET {
            assert_eq!(
                cached.get(property),
                live.get(property),
                "{} disagreed between a batched child and a live read",
                property.as_str()
            );
        }
    }
}

#[test]
fn a_collection_or_an_unknown_control_type_is_left_to_the_walk_one_by_one() {
    bootstrap();
    let fixture = HostedFixture::spawn().expect("the fixture host starts");
    let root = fixture_root(&fixture);
    let batch = batch();
    for id in COLLECTION_CONTROL_TYPES {
        assert!(
            batch.read(&root, &control_type(id), deadline()).is_none(),
            "control type {id} holds items, so nothing would bound the read"
        );
    }
    assert!(
        batch
            .read(
                &root,
                &ElementProperties::from_reads(Vec::new()),
                deadline()
            )
            .is_none()
    );
    assert!(batch.read(&root, &control_type(0), deadline()).is_none());
    assert!(
        batch
            .read(&root, &control_type(50033), deadline())
            .is_some()
    );
}

#[test]
fn a_batch_is_not_asked_once_the_deadline_has_passed() {
    bootstrap();
    let fixture = HostedFixture::spawn().expect("the fixture host starts");
    let expired = Deadline::after(1).expect("a short deadline");
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(
        batch()
            .read(&fixture_root(&fixture), &control_type(50033), expired)
            .is_none()
    );
}
