use super::scope_tests::{ScopeAdapter, landed_path, name_cell_of_row, resolve, rows};
use super::test_support::window;
use crate::Deadline;
use crate::locator::live_locator::ObservationRoot;
use crate::refs::refs_test_support::HomeGuard;

#[test]
fn a_twin_cell_reached_by_find_gets_the_chain_a_snapshot_gives_and_resolves() {
    use crate::locator::live_locator::{
        LocatorMaterialization, LocatorResolveRequest, LocatorSelection, evaluate_locator_tree,
    };
    use crate::locator::{IdentityPredicate, LocatorQuery};

    let _guard = HomeGuard::new();
    let tree = rows(&["a.txt", "b.txt", "c.txt"]);
    let from_snapshot = name_cell_of_row(tree.clone(), 1);
    let observed = crate::adapter::observed_tree(&ObservationRoot::Window(&window()), tree.clone())
        .expect("the fixture observes");
    let query = LocatorQuery {
        identity: IdentityPredicate {
            role: Some("button".into()),
            ..IdentityPredicate::default()
        },
        ..LocatorQuery::default()
    };
    let resolution = evaluate_locator_tree(
        observed,
        &query,
        &LocatorResolveRequest {
            selection: LocatorSelection::First,
            deadline: Deadline::standard().unwrap(),
            max_raw_depth: 50,
            surface: None,
            materialization: LocatorMaterialization::FullRefMap,
        },
    )
    .unwrap_or_else(|error| panic!("find evaluates: {error:?}"));
    let refmap = resolution.refmap.expect("find materializes the refs");
    let from_find = (1..=refmap.len())
        .filter_map(|index| refmap.get(&format!("@e{index}")).cloned())
        .find(|entry| {
            entry.identity.name.as_deref() == Some("Name") && entry.scope.path.as_slice() == [1, 0]
        })
        .expect("find publishes the twin cell");

    assert!(from_find.identity.proof.is_unique());
    assert_eq!(from_find.scope.chain, from_snapshot.scope.chain);
    let adapter = ScopeAdapter::new(tree);
    let handle = resolve(&adapter, &from_find).unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(landed_path(&handle), [1, 0]);
}

#[test]
fn a_truncated_observation_proves_nothing_and_resolves_nothing() {
    use crate::refs::ref_scope_resolve::{Miss, chain_nodes};

    let tree = rows(&["a.txt", "b.txt"]);
    let captured = name_cell_of_row(tree.clone(), 0);
    let mut observed = crate::adapter::observed_tree(&ObservationRoot::Window(&window()), tree)
        .expect("the fixture observes");
    let cut = observed
        .nodes
        .iter()
        .position(|node| node.evidence.name.known().map(String::as_str) == Some("b.txt"))
        .expect("the second row is observed");
    observed.nodes[cut].children.clear();
    observed.nodes[cut].children_count = Some(2);
    let cell = observed
        .nodes
        .iter()
        .position(|node| node.path.as_slice() == [0, 0])
        .expect("the first cell is observed");

    let mut entry = captured.clone();
    entry.identity.proof = crate::IdentityProof::Unproven;
    entry.scope.chain.clear();
    crate::refs::ref_capture_proof::CaptureProof::new(&observed).prove_node(&mut entry, cell);

    assert!(!entry.identity.proof.is_unique());
    assert!(entry.scope.chain.is_empty());
    assert_eq!(
        chain_nodes(&observed, &captured.scope.chain),
        Err(Miss::Unknown)
    );
}
