use super::{
    CaptureProof, LocatorEvidence, LocatorField, LocatorStats, ObservationSource, ObservedSubtree,
    ObservedTree, RefMap, allocate_refs, config, evidence, fixture_window, named, window,
};
use crate::AccessibilityNode;

fn textfields_with_unread_values(count: usize) -> (AccessibilityNode, ObservedTree) {
    let field = || {
        let mut evidence = evidence("textfield", "old text", Vec::new(), 0);
        evidence.value = LocatorField::Unknown;
        ObservedSubtree::new(evidence, Vec::new(), true, None)
    };
    let observed = ObservedTree::from_roots(
        vec![ObservedSubtree::new(
            evidence("window", "Fixture", Vec::new(), 0),
            (0..count).map(|_| field()).collect(),
            true,
            None,
        )],
        ObservationSource::Window {
            window: fixture_window(),
            surface: crate::SnapshotSurface::Window,
            renderer_accessibility_forced: false,
        },
        LocatorStats::default(),
        true,
    )
    .expect("the fixture observes");
    let fields = (0..count).map(|_| named("textfield", "old text")).collect();
    (window(fields), observed)
}

fn captured_entries(count: usize) -> Vec<crate::RefEntry> {
    let (root, observed) = textfields_with_unread_values(count);
    let proof = CaptureProof::new(&observed);
    let mut refmap = RefMap::new();
    allocate_refs(root, &mut refmap, &config(&proof)).unwrap();
    (1..=refmap.len())
        .filter_map(|index| refmap.get(&format!("@e{index}")).cloned())
        .filter(|entry| entry.identity.role == "textfield")
        .collect()
}

#[test]
fn a_name_on_a_field_with_an_unread_value_is_not_recorded_as_identity() {
    let entries = captured_entries(1);

    assert_eq!(entries[0].identity.name, None);
}

#[test]
fn a_field_with_an_unread_value_resolves_after_its_text_is_edited() {
    use crate::refs::ref_full_predicate::FullPredicate;
    use crate::refs::ref_live_predicate::LivePredicate;

    let entries = captured_entries(1);
    let mut edited: LocatorEvidence = evidence("textfield", "new text", Vec::new(), 0);
    edited.value = LocatorField::Unknown;
    let predicate = FullPredicate::of_entry(&entries[0]);

    assert_eq!(
        predicate.compare(&LivePredicate::observed(&edited)),
        crate::IdentityMatch::Match
    );
    assert!(entries[0].identity.proof.is_unique());
}

#[test]
fn twin_fields_with_unread_values_stay_unproven() {
    let entries = captured_entries(2);

    assert_eq!(entries.len(), 2);
    assert!(
        entries
            .iter()
            .all(|entry| !entry.identity.proof.is_unique())
    );
}

#[test]
fn a_found_field_with_an_unread_value_does_not_record_its_name() {
    let (_, observed) = textfields_with_unread_values(1);
    let field = observed
        .nodes
        .iter()
        .find(|node| node.evidence.role.known().map(String::as_str) == Some("textfield"))
        .expect("the fixture holds a field");
    let entry = crate::locator::live_locator::materialize_ref_entry(
        field,
        &observed.source,
        &crate::locator::LocatorQuery::default(),
    );

    assert_eq!(entry.identity.name, None);
}
