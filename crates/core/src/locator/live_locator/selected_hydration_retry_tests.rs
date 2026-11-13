use super::test_support::window;
use super::{
    IdentifierEvidence, LocatorField, LocatorMaterialization, LocatorResolveRequest,
    LocatorSelection, ObservationRequest, ObservationRoot, ObservationSource, ObservedTree,
    resolve_query,
};
use crate::{
    AdapterError, LocatorQuery, NativeHandle, Rect,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TransientHydrationAdapter {
    window_observations: AtomicUsize,
    strict_resolutions: AtomicUsize,
    hydration_observations: AtomicUsize,
}

impl ObservationOps for TransientHydrationAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        match root {
            ObservationRoot::Application { .. } => panic!("expected a window or element root"),
            ObservationRoot::Window(_) => {
                self.window_observations.fetch_add(1, Ordering::SeqCst);
                Ok(initial_tree())
            }
            ObservationRoot::Element { .. } => {
                let attempt = self.hydration_observations.fetch_add(1, Ordering::SeqCst);
                let root_evidence = request.evidence_for_raw_depth(0);
                assert!(root_evidence.states);
                assert!(root_evidence.ref_evidence.bounds);
                assert!(root_evidence.ref_evidence.actions);
                Ok(hydrated_tree(root, attempt == 0))
            }
        }
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &crate::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.strict_resolutions.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::null())
    }
}

impl ActionOps for TransientHydrationAdapter {}
impl InputOps for TransientHydrationAdapter {}
impl SystemOps for TransientHydrationAdapter {}

#[test]
fn one_transient_read_during_hydration_is_retried_and_the_find_succeeds() {
    let adapter = TransientHydrationAdapter {
        window_observations: AtomicUsize::new(0),
        strict_resolutions: AtomicUsize::new(0),
        hydration_observations: AtomicUsize::new(0),
    };
    let query = LocatorQuery {
        identity: crate::IdentityPredicate {
            role: Some("button".into()),
            name: Some("Save".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline: crate::Deadline::after(5_000).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    };

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("a single transient hydration read failure is retried and succeeds");

    assert_eq!(resolution.matches.len(), 1);
    assert_eq!(resolution.matches[0].data.name, "Save");
    assert_eq!(adapter.hydration_observations.load(Ordering::SeqCst), 2);
}

fn initial_tree() -> ObservedTree {
    let mut evidence = base_evidence(LocatorField::Known("Save".into()));
    evidence.states = LocatorField::Unknown;
    evidence.ref_evidence.available_actions = LocatorField::Unknown;
    super::test_support::tree(
        vec![super::test_support::node(0, evidence, Vec::new(), &[])],
        vec![0],
        true,
    )
}

fn hydrated_tree(root: ObservationRoot<'_>, transient_failure: bool) -> ObservedTree {
    let name = if transient_failure {
        LocatorField::Unknown
    } else {
        LocatorField::Known("Save".into())
    };
    let evidence = base_evidence(name);
    let mut tree = super::test_support::tree(
        vec![super::test_support::node(0, evidence, Vec::new(), &[])],
        vec![0],
        true,
    );
    if transient_failure {
        tree.stats.reads.health.cannot_complete = 1;
    }
    tree.source = ObservationSource::from_root(&root, crate::SnapshotSurface::Window);
    tree
}

fn base_evidence(name: LocatorField<String>) -> super::LocatorEvidence {
    let mut evidence = super::test_support::evidence("button", None);
    evidence.name = name;
    evidence.description = LocatorField::Absent;
    evidence.identifiers = IdentifierEvidence::absent();
    evidence.states = LocatorField::Known(Vec::new());
    evidence.ref_evidence.bounds = LocatorField::Known(Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 30.0,
    });
    evidence.ref_evidence.available_actions =
        LocatorField::Known(vec![crate::actions::capability::CLICK.into()]);
    evidence
}
