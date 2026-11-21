use std::sync::Arc;

use crate::adapter::{SnapshotSurface, minimal_accessibility_node, observed_tree};
use crate::locator::live_locator::{ObservationRoot, materialize_ref_entry};
use crate::refs::ref_capture_proof::CaptureProof;
use crate::refs::scope_chain::MAX_SCOPE_MEMORY_BYTES;
use crate::{
    AccessibilityNode, LocatorQuery, ProcessId, RefCapabilities, RefEntry, RefEntryIdentity,
    RefGeometry, RefMap, RefProcess, RefScope, RefSource, ScopeChain, WindowInfo, WindowState,
};

fn fixture_window() -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Fixture".into(),
        app: "Fixture".into(),
        pid: ProcessId::new(1),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: WindowState::default(),
    }
}

fn node(role: &str, name: &str, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut item = minimal_accessibility_node(role, name);
    item.children = children;
    item
}

fn minimal_entry(role: &str) -> RefEntry {
    RefEntry {
        process: RefProcess {
            pid: ProcessId::new(1),
            process_instance: Some("test-instance".into()),
        },
        identity: RefEntryIdentity {
            role: role.into(),
            unique_at_capture: true,
            ..Default::default()
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
            renderer_accessibility_forced: false,
        },
        scope: RefScope::default(),
    }
}

fn mirrored_branch(
    tag: &str,
    levels: usize,
    leaves: usize,
    label_bytes: usize,
) -> AccessibilityNode {
    let mut current: Vec<AccessibilityNode> = (0..leaves)
        .map(|idx| node("button", &format!("Action {idx}"), Vec::new()))
        .collect();
    for level in (0..levels).rev() {
        let prefix = if level == 0 {
            format!("{tag}-L0-")
        } else {
            format!("shared-L{level}-")
        };
        let filler = "x".repeat(label_bytes.saturating_sub(prefix.len()));
        let group_name = format!("{prefix}{filler}");
        current = vec![node("group", &group_name, current)];
    }
    current.pop().expect("non-empty branch")
}

#[test]
fn deep_mirrored_hierarchy_shares_interned_scope_chains_without_amplification() {
    let window = fixture_window();
    let left = mirrored_branch("left", 8, 500, 48 * 1024);
    let right = mirrored_branch("right", 8, 500, 48 * 1024);
    let root = node("window", "Fixture", vec![left, right]);
    let observed = observed_tree(&ObservationRoot::Window(&window), root).expect("observed tree");

    let proof = CaptureProof::new(&observed);
    let mut left_chains: Vec<ScopeChain> = Vec::new();
    let mut right_chains: Vec<ScopeChain> = Vec::new();
    for (index, item) in observed.nodes.iter().enumerate() {
        if item.evidence.role.known().map(String::as_str) != Some("button") {
            continue;
        }
        let mut entry = materialize_ref_entry(item, &observed.source, &LocatorQuery::default());
        proof.prove_node(&mut entry, index);
        assert!(entry.identity.unique_at_capture);
        assert_eq!(entry.scope.chain.len(), 8);
        let root_name = entry.scope.chain[7].name.as_deref().unwrap_or("");
        if root_name.starts_with("left-") {
            left_chains.push(entry.scope.chain);
        } else {
            right_chains.push(entry.scope.chain);
        }
    }

    assert_eq!(left_chains.len(), 500);
    assert_eq!(right_chains.len(), 500);
    for chain in &left_chains[1..] {
        assert!(left_chains[0].ptr_eq(chain));
    }
    for chain in &right_chains[1..] {
        assert!(right_chains[0].ptr_eq(chain));
    }
    for level in 0..7 {
        assert!(Arc::ptr_eq(&left_chains[0][level], &right_chains[0][level]));
    }
    assert!(proof.scope_memory_bytes() <= MAX_SCOPE_MEMORY_BYTES);
}

#[test]
fn capture_proof_stops_allocating_chains_when_aggregate_memory_budget_is_reached() {
    let window = fixture_window();
    let label_len = 60 * 1024;
    let group_count = (MAX_SCOPE_MEMORY_BYTES / label_len) + 6;
    let groups: Vec<AccessibilityNode> = (0..group_count)
        .map(|idx| {
            let prefix = format!("scope-{idx:03}-");
            let name = format!("{prefix}{}", "a".repeat(label_len - prefix.len()));
            node(
                "group",
                &name,
                vec![node("button", "Duplicate", Vec::new())],
            )
        })
        .collect();
    let root = node("window", "Fixture", groups);
    let observed = observed_tree(&ObservationRoot::Window(&window), root).expect("observed tree");

    let proof = CaptureProof::new(&observed);
    let mut proven = 0usize;
    let mut refused = 0usize;
    for (index, item) in observed.nodes.iter().enumerate() {
        if item.evidence.role.known().map(String::as_str) != Some("button") {
            continue;
        }
        let mut entry = materialize_ref_entry(item, &observed.source, &LocatorQuery::default());
        proof.prove_node(&mut entry, index);
        if entry.identity.unique_at_capture {
            proven += 1;
        } else {
            assert!(entry.scope.chain.is_empty());
            refused += 1;
        }
    }
    assert!(proven > 0);
    assert!(refused > 0);
    assert!(proof.scope_memory_bytes() <= MAX_SCOPE_MEMORY_BYTES);
}

#[test]
fn ref_map_deserialization_shares_chains_in_memory_and_enforces_memory_budget() {
    let big_label = "z".repeat(48 * 1024);
    let shared_chain = ScopeChain::from_owned(vec![RefEntryIdentity {
        role: "group".into(),
        name: Some(big_label),
        ..Default::default()
    }]);
    let mut map = RefMap::new();
    for _ in 0..200 {
        let mut entry = minimal_entry("button");
        entry.scope.chain = shared_chain.clone();
        map.try_allocate(entry).expect("allocate");
    }

    let json = map.serialize_with_size_check().expect("fits wire budget");
    let restored: RefMap = serde_json::from_str(&json).expect("deserialize shared chains");
    let first = &restored.get("@e1").expect("first").scope.chain;
    for idx in 2..=200 {
        let other = &restored
            .get(&format!("@e{idx}"))
            .expect("entry")
            .scope
            .chain;
        assert!(first.ptr_eq(other));
        assert!(Arc::ptr_eq(&first[0], &other[0]));
    }

    let oversized_chains: Vec<Vec<RefEntryIdentity>> = (0..22)
        .map(|idx| {
            vec![RefEntryIdentity {
                role: "group".into(),
                name: Some(format!("{idx:02}-{}", "q".repeat(55 * 1024))),
                ..Default::default()
            }]
        })
        .collect();
    let oversized_wire = serde_json::json!({
        "inner": { "@e1": serde_json::to_value(minimal_entry("button")).expect("entry") },
        "counter": 1,
        "chains": oversized_chains,
        "chain_of": { "@e1": 0 }
    });
    let error = serde_json::from_value::<RefMap>(oversized_wire)
        .expect_err("oversized scope chains must be rejected before expansion");
    assert!(error.to_string().contains("memory budget"), "error={error}");
}
