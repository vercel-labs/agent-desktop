use std::sync::Mutex;

use super::{ElementAtArgs, attach_completeness, candidates_at, execute, pick};
use crate::{
    AccessibilityNode, AdapterError, Deadline, HitTestResult, NativeHandle, NodeIdentity,
    NodePresentation, Point, ProcessId, Rect, RefEntry, WindowInfo,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps, WindowFilter},
    context::CommandContext,
    locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree},
};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn node(
    ref_id: Option<&str>,
    bounds: Option<Rect>,
    children: Vec<AccessibilityNode>,
) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: ref_id.map(str::to_string),
        role: "button".into(),
        identity: Default::default(),
        presentation: NodePresentation {
            bounds,
            ..Default::default()
        },
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

fn window() -> AccessibilityNode {
    node(
        None,
        Some(rect(0.0, 0.0, 800.0, 600.0)),
        vec![
            node(
                Some("@s1:e1"),
                Some(rect(100.0, 100.0, 400.0, 300.0)),
                vec![
                    node(Some("@s1:e2"), Some(rect(120.0, 120.0, 80.0, 30.0)), vec![]),
                    node(None, Some(rect(120.0, 120.0, 10.0, 10.0)), vec![]),
                ],
            ),
            node(Some("@s1:e3"), Some(rect(600.0, 100.0, 50.0, 50.0)), vec![]),
            node(Some("@s1:e4"), None, vec![]),
        ],
    )
}

fn refs(candidates: &[(&AccessibilityNode, Rect)]) -> Vec<String> {
    candidates
        .iter()
        .filter_map(|(node, _)| node.ref_id.clone())
        .collect()
}

#[test]
fn only_refs_containing_the_point_are_candidates_smallest_first() {
    let tree = window();
    let candidates = candidates_at(&tree, &Point { x: 130.0, y: 130.0 });
    assert_eq!(refs(&candidates), ["@s1:e2", "@s1:e1"]);
}

#[test]
fn a_point_on_the_far_edge_is_outside() {
    let tree = window();
    assert!(candidates_at(&tree, &Point { x: 650.0, y: 120.0 }).is_empty());
    assert_eq!(
        refs(&candidates_at(&tree, &Point { x: 649.0, y: 120.0 })),
        ["@s1:e3"]
    );
}

#[test]
fn equal_areas_put_the_deeper_element_first() {
    let tree = node(
        None,
        None,
        vec![node(
            Some("@s1:e1"),
            Some(rect(0.0, 0.0, 50.0, 50.0)),
            vec![node(
                Some("@s1:e2"),
                Some(rect(0.0, 0.0, 50.0, 50.0)),
                vec![],
            )],
        )],
    );
    assert_eq!(
        refs(&candidates_at(&tree, &Point { x: 10.0, y: 10.0 })),
        ["@s1:e2", "@s1:e1"]
    );
}

#[test]
fn the_first_candidate_the_point_reaches_is_chosen() {
    let tree = window();
    let candidates = candidates_at(&tree, &Point { x: 130.0, y: 130.0 });
    let chosen = pick(&candidates, |node| {
        Ok(match node.ref_id.as_deref() {
            Some("@s1:e1") => HitTestResult::ReachesTarget,
            _ => HitTestResult::Unknown,
        })
    });
    assert_eq!(chosen, Some((1, HitTestResult::ReachesTarget)));
}

#[test]
fn without_proof_the_smallest_is_returned_with_what_its_hit_test_saw() {
    let tree = window();
    let candidates = candidates_at(&tree, &Point { x: 130.0, y: 130.0 });
    let occluder = HitTestResult::InterceptedBy {
        role: Some("window".into()),
        name: Some("Other".into()),
        bounds: None,
    };
    let chosen = pick(&candidates, |_| Ok(occluder.clone()));
    assert_eq!(chosen, Some((0, occluder)));
}

#[test]
fn a_probe_error_counts_as_unknown() {
    let tree = window();
    let candidates = candidates_at(&tree, &Point { x: 130.0, y: 130.0 });
    let chosen = pick(&candidates, |_| {
        Err(AdapterError::not_supported("hit_test"))
    });
    assert_eq!(chosen, Some((0, HitTestResult::Unknown)));
}

#[test]
fn no_candidate_means_no_match() {
    assert_eq!(pick(&[], |_| Ok(HitTestResult::ReachesTarget)), None);
}

#[test]
fn a_complete_snapshot_says_so_without_partial_fields() {
    let mut response = serde_json::json!({ "match": null });
    attach_completeness(&mut response, true, 42);
    assert_eq!(response["complete"], true);
    assert!(response.get("truncated").is_none());
    assert!(response.get("nodes_observed").is_none());
}

#[test]
fn a_partial_snapshot_is_reported_like_snapshot_reports_it() {
    let mut response = serde_json::json!({ "match": null });
    attach_completeness(&mut response, false, 17);
    assert_eq!(response["complete"], false);
    assert_eq!(response["truncated"], true);
    assert_eq!(response["nodes_observed"], 17);
}

struct TokenlessHitTestAdapter {
    tree: AccessibilityNode,
    tested: Mutex<Vec<String>>,
}

impl ObservationOps for TokenlessHitTestAdapter {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Fixture".into(),
            app: "Fixture".into(),
            pid: ProcessId::new(1),
            process_instance: Some("inst-1".into()),
            bounds: Some(rect(0.0, 0.0, 800.0, 600.0)),
            state: crate::WindowState::default(),
        }])
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        match root {
            ObservationRoot::Element { .. } => crate::adapter::saved_ref_element_tree(root),
            _ => crate::adapter::observed_tree(&root, self.tree.clone()),
        }
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        let name = entry.identity.name.clone().unwrap_or_default();
        Ok(NativeHandle::new(name))
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        _point: Point,
        _deadline: Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        let name = handle.downcast_ref::<String>().cloned().unwrap_or_default();
        self.tested.lock().unwrap().push(name.clone());
        if name == "Outer" {
            Ok(HitTestResult::ReachesTarget)
        } else {
            Ok(HitTestResult::InterceptedBy {
                role: Some("button".into()),
                name: Some("Outer".into()),
                bounds: None,
            })
        }
    }
}

impl ActionOps for TokenlessHitTestAdapter {}
impl InputOps for TokenlessHitTestAdapter {}
impl SystemOps for TokenlessHitTestAdapter {}

fn named_button(name: &str, bounds: Rect, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: "button".into(),
        identity: NodeIdentity {
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: NodePresentation {
            bounds: Some(bounds),
            ..Default::default()
        },
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

#[test]
fn tokenless_adapter_without_resolve_element_strict_still_hit_tests_candidates() {
    let tree = AccessibilityNode {
        ref_id: None,
        role: "window".into(),
        identity: NodeIdentity {
            name: Some("Fixture".into()),
            ..Default::default()
        },
        presentation: NodePresentation {
            bounds: Some(rect(0.0, 0.0, 800.0, 600.0)),
            ..Default::default()
        },
        children_count: None,
        subtree_truncated: false,
        children: vec![named_button(
            "Outer",
            rect(100.0, 100.0, 200.0, 120.0),
            vec![named_button(
                "Inner",
                rect(120.0, 120.0, 80.0, 40.0),
                vec![],
            )],
        )],
    };
    let adapter = TokenlessHitTestAdapter {
        tree,
        tested: Mutex::new(Vec::new()),
    };
    let response = execute(
        ElementAtArgs {
            app: Some("Fixture".into()),
            window_id: None,
            point: Point { x: 130.0, y: 130.0 },
            max_depth: 50,
            timeout_ms: Some(5_000),
        },
        &adapter,
        &CommandContext::default(),
    )
    .expect("element-at succeeds on tokenless adapter");
    assert_eq!(response["match"]["name"], "Outer");
    assert_eq!(response["hit"], "reaches_target");
    assert_eq!(*adapter.tested.lock().unwrap(), vec!["Inner", "Outer"]);
}
