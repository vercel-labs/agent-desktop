use super::*;
use crate::snapshot::SnapshotResult;
use crate::snapshot::baseline::{SnapshotBaseline, redact_sensitive_tree_values};
use crate::{
    ElementIdentifier, IdentifierKind, NodeDescriptor, NodeIdentity, NodePresentation, ProcessId,
    Rect, RefMap, SnapshotOwner, WindowInfo, WindowState, adapter::TreeOptions,
};

fn node(role: &str, name: Option<&str>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: NodeIdentity {
            name: name.map(str::to_owned),
            ..Default::default()
        },
        presentation: NodePresentation::default(),
        children_count: None,
        subtree_truncated: false,
        children: Vec::new(),
    }
}

fn make_baseline(tree: AccessibilityNode, snap_id: &str) -> (SnapshotBaseline, AccessibilityNode) {
    let mut result = SnapshotResult {
        tree,
        refmap: RefMap::new(),
        owner: SnapshotOwner::Window(WindowInfo {
            id: "w-1".into(),
            title: "App".into(),
            app: "App".into(),
            pid: ProcessId::new(1),
            process_instance: Some("inst-1".into()),
            bounds: None,
            state: WindowState::default(),
        }),
        snapshot_id: Some(snap_id.into()),
        complete: true,
        nodes_observed: 1,
        identity_unproven: None,
    };
    let raw_tree = result.tree.clone();
    let baseline = SnapshotBaseline::from_snapshot_result(&result, &TreeOptions::default(), None);
    redact_sensitive_tree_values(&mut result);
    (baseline, raw_tree)
}

#[test]
fn unchanged_trees_produce_empty_delta_even_when_snapshot_ids_differ() {
    let mut root1 = node("window", Some("Main"));
    let mut btn1 = node("button", Some("Save"));
    btn1.ref_id = Some("@s1:e1".into());
    root1.children.push(btn1);

    let mut root2 = node("window", Some("Main"));
    let mut btn2 = node("button", Some("Save"));
    btn2.ref_id = Some("@s2:e1".into());
    root2.children.push(btn2);

    let (b1, _) = make_baseline(root1, "s1");
    let (b2, raw2) = make_baseline(root2, "s2");
    assert_eq!(b1.tree_hash, b2.tree_hash);
    let delta = diff_baselines(
        BaselineRef {
            snapshot_id: "s1",
            baseline: &b1,
        },
        BaselineRef {
            snapshot_id: "s2",
            baseline: &b2,
        },
        &raw2,
    );
    assert!(delta.is_empty());
}

#[test]
fn updated_node_emits_explicit_null_when_optional_fields_clear() {
    let mut root1 = node("window", Some("Main"));
    let mut field1 = node("textfield", Some("Email"));
    field1.ref_id = Some("@s1:e1".into());
    field1.identity.value = Some("a@b.com".into());
    field1.identity.description = Some("User email".into());
    field1.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "email_input".into(),
    });
    field1.presentation.hint = Some("enter email".into());
    field1.presentation.states = vec!["checked".into(), "focused".into()];
    field1.presentation.available_actions = vec!["SetValue".into()];
    field1.presentation.bounds = Some(Rect {
        x: 1.0,
        y: 2.0,
        width: 100.0,
        height: 24.0,
    });
    field1.presentation.descriptors = NodeDescriptor {
        subrole: Some("search".into()),
        role_description: Some("text field".into()),
        placeholder: Some("you@example.com".into()),
        dom_classes: vec!["input".into()],
    };
    field1.children_count = Some(2);
    field1.subtree_truncated = true;
    root1.children.push(field1);

    let mut root2 = node("window", Some("Main"));
    let mut field2 = node("textfield", None);
    field2.ref_id = Some("@s2:e1".into());
    field2.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "email_input".into(),
    });
    root2.children.push(field2);

    let (b1, _) = make_baseline(root1, "s1");
    let (b2, raw2) = make_baseline(root2, "s2");
    let delta = diff_baselines(
        BaselineRef {
            snapshot_id: "s1",
            baseline: &b1,
        },
        BaselineRef {
            snapshot_id: "s2",
            baseline: &b2,
        },
        &raw2,
    );
    assert!(delta.added.is_empty());
    assert!(delta.removed.is_empty());
    assert_eq!(delta.updated.len(), 1);
    let updated = delta.updated[0].as_object().unwrap();
    assert_eq!(updated["path"], json!([0]));
    assert_eq!(updated["ref_id"], "@s2:e1");
    for cleared_opt in [
        "name",
        "value",
        "description",
        "hint",
        "bounds",
        "subrole",
        "role_description",
        "placeholder",
        "children_count",
    ] {
        assert_eq!(
            updated.get(cleared_opt),
            Some(&Value::Null),
            "{cleared_opt} must be explicit null"
        );
    }
    assert_eq!(updated["states"], json!([]));
    assert_eq!(updated["available_actions"], json!([]));
    assert_eq!(updated["dom_classes"], json!([]));
    assert_eq!(updated["subtree_truncated"], json!(false));
}

#[test]
fn sibling_index_shift_and_reorder_emit_remove_plus_add() {
    let mut root1 = node("window", Some("List"));
    let mut a1 = node("button", Some("Alice"));
    a1.ref_id = Some("@s1:e1".into());
    let mut b1 = node("button", Some("Bob"));
    b1.ref_id = Some("@s1:e2".into());
    root1.children = vec![a1, b1];

    let mut root2 = node("window", Some("List"));
    let mut b2 = node("button", Some("Bob"));
    b2.ref_id = Some("@s2:e1".into());
    root2.children = vec![b2];

    let (b1_snap, _) = make_baseline(root1, "s1");
    let (b2_snap, raw2) = make_baseline(root2, "s2");
    let delta = diff_baselines(
        BaselineRef {
            snapshot_id: "s1",
            baseline: &b1_snap,
        },
        BaselineRef {
            snapshot_id: "s2",
            baseline: &b2_snap,
        },
        &raw2,
    );
    assert!(delta.updated.is_empty());
    assert_eq!(delta.removed.len(), 2);
    assert_eq!(delta.added.len(), 1);
    assert_eq!(delta.removed[0]["path"], json!([0]));
    assert_eq!(delta.removed[0]["name"], "Alice");
    assert_eq!(delta.removed[1]["path"], json!([1]));
    assert_eq!(delta.removed[1]["name"], "Bob");
    assert_eq!(delta.added[0]["path"], json!([0]));
    assert_eq!(delta.added[0]["name"], "Bob");
    assert_eq!(delta.added[0]["ref_id"], "@s2:e1");
}

#[test]
fn role_change_at_same_path_emits_remove_plus_add() {
    let mut root1 = node("window", Some("Main"));
    root1.children.push(node("button", Some("Item")));

    let mut root2 = node("window", Some("Main"));
    root2.children.push(node("checkbox", Some("Item")));

    let (b1, _) = make_baseline(root1, "s1");
    let (b2, raw2) = make_baseline(root2, "s2");
    let delta = diff_baselines(
        BaselineRef {
            snapshot_id: "s1",
            baseline: &b1,
        },
        BaselineRef {
            snapshot_id: "s2",
            baseline: &b2,
        },
        &raw2,
    );
    assert!(delta.updated.is_empty());
    assert_eq!(delta.removed.len(), 1);
    assert_eq!(delta.removed[0]["role"], "button");
    assert_eq!(delta.added.len(), 1);
    assert_eq!(delta.added[0]["role"], "checkbox");
}

#[test]
fn secure_node_hashes_before_redaction_and_never_leaks_raw_values() {
    let mut root1 = node("window", Some("Auth"));
    let mut pw1 = node("textfield", Some("Password"));
    pw1.ref_id = Some("@s1:e1".into());
    pw1.identity.value = Some("hunter2-secret".into());
    pw1.presentation.states = vec!["secure".into()];
    root1.children.push(pw1);

    let mut root2 = node("window", Some("Auth"));
    let mut pw2 = node("textfield", Some("Password"));
    pw2.ref_id = Some("@s2:e1".into());
    pw2.identity.value = Some("different-secret".into());
    pw2.presentation.states = vec!["secure".into()];
    root2.children.push(pw2);

    let (b1, _) = make_baseline(root1.clone(), "s1");
    let (b2, raw2) = make_baseline(root2, "s2");
    let serialized_b1 = serde_json::to_string(&b1).unwrap();
    let serialized_b2 = serde_json::to_string(&b2).unwrap();
    assert!(!serialized_b1.contains("hunter2-secret"));
    assert!(!serialized_b2.contains("different-secret"));
    assert_ne!(b1.tree_hash, b2.tree_hash);

    let delta = diff_baselines(
        BaselineRef {
            snapshot_id: "s1",
            baseline: &b1,
        },
        BaselineRef {
            snapshot_id: "s2",
            baseline: &b2,
        },
        &raw2,
    );
    let delta_json = serde_json::to_string(&delta).unwrap();
    assert!(!delta_json.contains("hunter2-secret"));
    assert!(!delta_json.contains("different-secret"));
    assert_eq!(delta.updated.len(), 1);
    assert_eq!(delta.updated[0]["value"], json!({ "redacted": true }));

    let mut root3 = node("window", Some("Auth"));
    let mut pw3 = node("textfield", Some("Password"));
    pw3.ref_id = Some("@s3:e1".into());
    pw3.presentation.states = vec!["secure".into()];
    root3.children.push(pw3);
    let (b3, raw3) = make_baseline(root3, "s3");
    let cleared_delta = diff_baselines(
        BaselineRef {
            snapshot_id: "s1",
            baseline: &b1,
        },
        BaselineRef {
            snapshot_id: "s3",
            baseline: &b3,
        },
        &raw3,
    );
    assert_eq!(cleared_delta.updated[0]["value"], Value::Null);
}

#[test]
fn unchanged_node_whose_local_ref_shifted_emits_fresh_ref_id() {
    let mut root1 = node("window", Some("Main"));
    let grp1 = node("group", Some("Header"));
    let mut btn1 = node("button", Some("Next"));
    btn1.ref_id = Some("@s1:e1".into());
    root1.children = vec![grp1, btn1];

    let mut root2 = node("window", Some("Main"));
    let mut grp2 = node("button", Some("Header"));
    grp2.ref_id = Some("@s2:e1".into());
    let mut btn2 = node("button", Some("Next"));
    btn2.ref_id = Some("@s2:e2".into());
    root2.children = vec![grp2, btn2];

    let (b1, _) = make_baseline(root1, "s1");
    let (b2, raw2) = make_baseline(root2, "s2");
    let delta = diff_baselines(
        BaselineRef {
            snapshot_id: "s1",
            baseline: &b1,
        },
        BaselineRef {
            snapshot_id: "s2",
            baseline: &b2,
        },
        &raw2,
    );
    assert_eq!(delta.updated.len(), 1);
    assert_eq!(delta.updated[0]["path"], json!([1]));
    assert_eq!(delta.updated[0]["ref_id"], "@s2:e2");
}
