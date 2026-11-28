use super::test_support::window;
use super::{
    IdentifierEvidence, LocatorField, LocatorMaterialization, LocatorResolveRequest,
    LocatorSelection, ObservationRequest, ObservationRoot, ObservationSource, ObservedTree,
    resolve_query,
};
use crate::{
    AdapterError, ContainmentPredicate, LocatorQuery, NativeHandle, Rect,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy)]
enum Subtree {
    HasText,
    Has,
    HasNot,
}

struct SubtreeHydrationAdapter {
    subtree: Subtree,
    window_observations: AtomicUsize,
    selected_observations: AtomicUsize,
}

impl ObservationOps for SubtreeHydrationAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        match root {
            ObservationRoot::Application { .. } => panic!("expected a window or element root"),
            ObservationRoot::Window(_) => {
                self.window_observations.fetch_add(1, Ordering::SeqCst);
                Ok(initial_tree(self.subtree))
            }
            ObservationRoot::Element { .. } => {
                self.selected_observations.fetch_add(1, Ordering::SeqCst);
                assert_eq!(request.max_raw_depth, 1);
                assert_eq!(request.max_logical_depth, 0);
                assert_eq!(
                    request.evidence_for_raw_depth(0),
                    super::EvidenceRequirements::snapshot()
                );
                Ok(selected_tree(root))
            }
        }
    }

    fn resolve_locator_anchor(
        &self,
        entry: &crate::RefEntry,
        _base: Option<&NativeHandle>,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        assert_eq!(entry.geometry.bounds, Some(root_bounds()));
        Ok(NativeHandle::null())
    }
}

impl ActionOps for SubtreeHydrationAdapter {}
impl InputOps for SubtreeHydrationAdapter {}
impl SystemOps for SubtreeHydrationAdapter {}

#[test]
fn a_subtree_predicate_hydrates_only_the_anchor_it_already_proved() {
    for subtree in [Subtree::HasText, Subtree::Has, Subtree::HasNot] {
        let adapter = SubtreeHydrationAdapter {
            subtree,
            window_observations: AtomicUsize::new(0),
            selected_observations: AtomicUsize::new(0),
        };
        let resolution = resolve_query(
            &adapter,
            &query(subtree),
            ObservationRoot::Window(&window()),
            &request(),
        )
        .expect("the anchor the whole-window read matched hydrates on its own");

        assert_eq!(adapter.window_observations.load(Ordering::SeqCst), 1);
        assert_eq!(adapter.selected_observations.load(Ordering::SeqCst), 1);
        assert_eq!(resolution.matches.len(), 1);
    }
}

fn initial_tree(subtree: Subtree) -> ObservedTree {
    let child = match subtree {
        Subtree::HasText => text_evidence("needle"),
        Subtree::Has => role_evidence("button"),
        Subtree::HasNot => role_evidence("link"),
    };
    super::test_support::tree(
        vec![
            super::test_support::node(0, phase_one_root(), vec![1], &[]),
            super::test_support::node(1, child, Vec::new(), &[0]),
        ],
        vec![0],
        true,
    )
}

fn selected_tree(root: ObservationRoot<'_>) -> ObservedTree {
    let mut tree = super::test_support::tree(
        vec![super::test_support::node(
            0,
            hydrated_root(),
            Vec::new(),
            &[],
        )],
        vec![0],
        true,
    );
    tree.source = ObservationSource::from_root(&root, crate::SnapshotSurface::Window);
    tree
}

fn phase_one_root() -> super::LocatorEvidence {
    let mut evidence = role_evidence("group");
    evidence.identifiers = IdentifierEvidence::absent();
    evidence.ref_evidence.bounds = LocatorField::Known(root_bounds());
    evidence
}

fn hydrated_root() -> super::LocatorEvidence {
    let mut evidence = super::test_support::evidence("group", None);
    evidence.ref_evidence.bounds = LocatorField::Known(root_bounds());
    evidence
}

fn role_evidence(role: &str) -> super::LocatorEvidence {
    let mut evidence = super::test_support::evidence(role, None);
    evidence.name = LocatorField::Unknown;
    evidence.description = LocatorField::Unknown;
    evidence.value = LocatorField::Unknown;
    evidence.identifiers = IdentifierEvidence::unknown();
    evidence.states = LocatorField::Unknown;
    evidence.ref_evidence.bounds = LocatorField::Unknown;
    evidence.ref_evidence.available_actions = LocatorField::Unknown;
    evidence
}

fn text_evidence(text: &str) -> super::LocatorEvidence {
    let mut evidence = role_evidence("statictext");
    evidence.name = LocatorField::Known(text.into());
    evidence.description = LocatorField::Absent;
    evidence.value = LocatorField::Absent;
    evidence
}

fn query(subtree: Subtree) -> LocatorQuery {
    let mut query = LocatorQuery {
        identity: crate::IdentityPredicate {
            role: Some("group".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    match subtree {
        Subtree::HasText => query.has_text = Some("needle".into()),
        Subtree::Has => query.containment = containment(true),
        Subtree::HasNot => query.containment = containment(false),
    }
    query
}

fn containment(positive: bool) -> ContainmentPredicate {
    let nested = Box::new(LocatorQuery {
        identity: crate::IdentityPredicate {
            role: Some("button".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    if positive {
        ContainmentPredicate {
            has: Some(nested),
            has_not: None,
        }
    } else {
        ContainmentPredicate {
            has: None,
            has_not: Some(nested),
        }
    }
}

fn request() -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline: crate::Deadline::after(5_000).unwrap(),
        max_raw_depth: 10,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    }
}

fn root_bounds() -> Rect {
    Rect {
        x: 10.0,
        y: 20.0,
        width: 300.0,
        height: 200.0,
    }
}
