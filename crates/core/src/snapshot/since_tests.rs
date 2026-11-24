use crate::adapter::{
    ActionOps, InputOps, ObservationOps, SnapshotSurface, SystemOps, WindowFilter,
};
use crate::commands::snapshot::{SnapshotArgs, execute};
use crate::context::{CommandContext, WaitSelector};
use crate::refs::refs_store::RefStore;
use crate::refs::refs_test_support::HomeGuard;
use crate::{AccessibilityNode, AdapterError, ErrorCode, ProcessId, WindowInfo, WindowState};
use std::sync::{Arc, Mutex};

struct MutableSnapshotAdapter {
    tree: Arc<Mutex<AccessibilityNode>>,
    window_id: Arc<Mutex<String>>,
    app_name: Arc<Mutex<String>>,
}

impl MutableSnapshotAdapter {
    fn new(tree: AccessibilityNode) -> Self {
        Self {
            tree: Arc::new(Mutex::new(tree)),
            window_id: Arc::new(Mutex::new("w-1".into())),
            app_name: Arc::new(Mutex::new("FixtureApp".into())),
        }
    }
}

impl ObservationOps for MutableSnapshotAdapter {
    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        crate::adapter::observed_tree(&root, self.tree.lock().unwrap().clone())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: self.window_id.lock().unwrap().clone(),
            title: "Main".into(),
            app: self.app_name.lock().unwrap().clone(),
            pid: ProcessId::new(42),
            process_instance: Some("inst-42".into()),
            bounds: None,
            state: WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }
}

impl ActionOps for MutableSnapshotAdapter {}
impl InputOps for MutableSnapshotAdapter {}
impl SystemOps for MutableSnapshotAdapter {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window, SnapshotSurface::Menu]
    }
}

fn node(role: &str, name: Option<&str>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            name: name.map(str::to_owned),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: Vec::new(),
    }
}

fn sample_tree(checked: bool, text: Option<&str>, extra_button: bool) -> AccessibilityNode {
    let mut root = node("window", Some("Main"));
    let mut cb = node("checkbox", Some("Agree"));
    if checked {
        cb.presentation.states = vec!["checked".into()];
    }
    let mut tf = node("textfield", Some("Notes"));
    tf.identity.value = text.map(str::to_owned);
    root.children = vec![cb, tf];
    if extra_button {
        root.children.push(node("button", Some("Submit")));
    }
    root
}

fn args_with_since(since: Option<&str>) -> SnapshotArgs {
    SnapshotArgs {
        app: None,
        window_id: None,
        max_depth: 10,
        include_bounds: false,
        interactive_only: false,
        compact: false,
        surface: SnapshotSurface::Window,
        skeleton: false,
        root_ref: None,
        timeout_ms: None,
        force_electron_a11y: false,
        since: since.map(str::to_owned),
    }
}

#[test]
fn since_returns_unchanged_then_delta_and_keeps_fresh_refs_valid() {
    let _guard = HomeGuard::new();
    let adapter = MutableSnapshotAdapter::new(sample_tree(true, Some("hello"), false));
    let ctx = CommandContext::default();

    let s1 = execute(args_with_since(None), &adapter, &ctx).unwrap();
    let id1 = s1["snapshot_id"].as_str().unwrap().to_owned();
    assert!(s1.get("tree").is_some());
    assert!(s1.get("since_status").is_none());

    let s2 = execute(args_with_since(Some(&id1)), &adapter, &ctx).unwrap();
    let id2 = s2["snapshot_id"].as_str().unwrap().to_owned();
    assert_ne!(id1, id2);
    assert_eq!(s2["since"], id1);
    assert_eq!(s2["since_status"], "unchanged");
    assert_eq!(s2["unchanged"], true);
    assert!(s2.get("tree").is_none());
    assert!(s2.get("delta").is_none());
    assert_eq!(s2["ref_count"], 2);
    let store = RefStore::new().unwrap();
    assert!(store.load_ref(&format!("@{id2}:e1")).is_ok());

    *adapter.tree.lock().unwrap() = sample_tree(false, None, true);
    let s3 = execute(args_with_since(Some(&id2)), &adapter, &ctx).unwrap();
    let id3 = s3["snapshot_id"].as_str().unwrap().to_owned();
    assert_eq!(s3["since"], id2);
    assert_eq!(s3["since_status"], "delta");
    assert_eq!(s3["unchanged"], false);
    assert!(s3.get("tree").is_none());
    assert_eq!(s3["ref_count"], 3);
    assert_eq!(s3["delta"]["updated"].as_array().unwrap().len(), 2);
    assert_eq!(s3["delta"]["updated"][0]["states"], serde_json::json!([]));
    assert_eq!(s3["delta"]["updated"][1]["value"], serde_json::Value::Null);
    assert_eq!(s3["delta"]["added"].as_array().unwrap().len(), 1);
    assert_eq!(s3["delta"]["added"][0]["ref_id"], format!("@{id3}:e3"));
    assert!(store.load_ref(&format!("@{id3}:e3")).is_ok());
}

#[test]
fn since_falls_back_cleanly_for_unknown_expired_window_and_scope_mismatch() {
    let _guard = HomeGuard::new();
    let _no_fsync = crate::storage::private_file::without_fsync_for_test();
    let adapter = MutableSnapshotAdapter::new(sample_tree(false, None, false));
    let ctx = CommandContext::default();

    let unknown = execute(args_with_since(Some("smissing99")), &adapter, &ctx).unwrap();
    assert_eq!(unknown["since_status"], "unknown_baseline");
    assert!(unknown.get("tree").is_some());

    let id1 = unknown["snapshot_id"].as_str().unwrap().to_owned();
    let mut mismatch_scope = args_with_since(Some(&id1));
    mismatch_scope.interactive_only = true;
    let scope_res = execute(mismatch_scope, &adapter, &ctx).unwrap();
    assert_eq!(scope_res["since_status"], "scope_mismatch");
    assert!(scope_res.get("tree").is_some());

    *adapter.window_id.lock().unwrap() = "w-2".into();
    let win_res = execute(args_with_since(Some(&id1)), &adapter, &ctx).unwrap();
    assert_eq!(win_res["since_status"], "window_mismatch");
    assert!(win_res.get("tree").is_some());

    let store = RefStore::new().unwrap();
    for i in 0..130 {
        let mut map = crate::RefMap::new();
        let entry = crate::RefEntry {
            process: crate::RefProcess {
                pid: ProcessId::new(1),
                process_instance: Some("inst-1".into()),
            },
            identity: crate::RefEntryIdentity {
                retained_object: None,
                role: "button".into(),
                name: Some(format!("B{i}")),
                value: None,
                description: None,
                native_id: None,
                unique_at_capture: true,
            },
            geometry: crate::RefGeometry {
                bounds_space: None,
                bounds: None,
                bounds_hash: None,
            },
            capabilities: crate::RefCapabilities {
                states: vec![],
                available_actions: vec![],
            },
            source: crate::RefSource {
                source_app: Some("FixtureApp".into()),
                source_window_id: Some("w-2".into()),
                source_window_title: Some("Main".into()),
                source_window_bounds_hash: None,
                source_surface: SnapshotSurface::Window,
                renderer_accessibility_forced: false,
            },
            scope: crate::RefScope {
                root_ref: None,
                path_is_absolute: false,
                path: smallvec::SmallVec::new(),
            },
        };
        map.allocate(entry);
        store.save_new_snapshot(&map).unwrap();
    }
    let expired = execute(args_with_since(Some(&id1)), &adapter, &ctx).unwrap();
    assert_eq!(expired["since_status"], "expired");
    assert!(expired.get("tree").is_some());
}

#[test]
fn since_rejects_malformed_baseline_id_with_invalid_args() {
    let _guard = HomeGuard::new();
    let adapter = MutableSnapshotAdapter::new(sample_tree(false, None, false));
    let ctx = CommandContext::default();

    for bad in ["", "s1", "../escape", "@s8f3k2p9"] {
        let err = execute(args_with_since(Some(bad)), &adapter, &ctx).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgs.as_str(), "bad={bad}");
    }
}

#[test]
fn since_with_secure_field_detects_change_and_redacts_everywhere() {
    let _guard = HomeGuard::new();
    let mut root1 = node("window", Some("Login"));
    let mut pw1 = node("textfield", Some("Password"));
    pw1.identity.value = Some("top-secret-1".into());
    pw1.presentation.states = vec!["secure".into()];
    root1.children.push(pw1);

    let adapter = MutableSnapshotAdapter::new(root1);
    let ctx = CommandContext::default();
    let s1 = execute(args_with_since(None), &adapter, &ctx).unwrap();
    let id1 = s1["snapshot_id"].as_str().unwrap().to_owned();
    assert!(!s1.to_string().contains("top-secret-1"));

    let mut root2 = node("window", Some("Login"));
    let mut pw2 = node("textfield", Some("Password"));
    pw2.identity.value = Some("top-secret-2".into());
    pw2.presentation.states = vec!["secure".into()];
    root2.children.push(pw2);
    *adapter.tree.lock().unwrap() = root2;

    let s2 = execute(args_with_since(Some(&id1)), &adapter, &ctx).unwrap();
    let json_text = s2.to_string();
    assert_eq!(s2["since_status"], "delta");
    assert_eq!(s2["unchanged"], false);
    assert!(!json_text.contains("top-secret-1"));
    assert!(!json_text.contains("top-secret-2"));
    assert_eq!(
        s2["delta"]["updated"][0]["value"],
        serde_json::json!({ "redacted": true })
    );

    let store = RefStore::new().unwrap();
    let id2 = s2["snapshot_id"].as_str().unwrap();
    let raw_baseline = std::fs::read_to_string(store.baseline_path(id2)).unwrap();
    assert!(!raw_baseline.contains("top-secret-2"));
}

#[test]
fn since_works_with_wait_for_selector() {
    let _guard = HomeGuard::new();
    let adapter = MutableSnapshotAdapter::new(sample_tree(false, None, true));
    let ctx = CommandContext::default();
    let s1 = execute(args_with_since(None), &adapter, &ctx).unwrap();
    let id1 = s1["snapshot_id"].as_str().unwrap().to_owned();

    let wait_ctx = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: "button:Submit".into(),
        gone: false,
        timeout_ms: 2_000,
    }));
    let s2 = execute(args_with_since(Some(&id1)), &adapter, &wait_ctx).unwrap();
    assert_eq!(s2["since_status"], "unchanged");
    assert_eq!(s2["unchanged"], true);
    assert_eq!(s2["matched_selector"], "button:Submit");
    assert!(s2.get("elapsed_ms").is_some());
}

#[test]
fn since_falls_back_cleanly_for_incomplete_observation() {
    let _guard = HomeGuard::new();
    let adapter = MutableSnapshotAdapter::new(sample_tree(false, None, false));
    let ctx = CommandContext::default();
    let s1 = execute(args_with_since(None), &adapter, &ctx).unwrap();
    let id1 = s1["snapshot_id"].as_str().unwrap().to_owned();

    let mut incomplete_tree = sample_tree(false, None, false);
    incomplete_tree.subtree_truncated = true;
    let store = RefStore::new().unwrap();
    let partial = crate::snapshot::SnapshotResult {
        tree: incomplete_tree,
        refmap: crate::RefMap::new(),
        owner: crate::SnapshotOwner::Window(WindowInfo {
            id: "w-1".into(),
            title: "Main".into(),
            app: "FixtureApp".into(),
            pid: ProcessId::new(42),
            process_instance: Some("inst-42".into()),
            bounds: None,
            state: WindowState::default(),
        }),
        snapshot_id: None,
        complete: false,
        nodes_observed: 1,
        identity_unproven: None,
    };
    let partial_baseline = crate::snapshot::baseline::SnapshotBaseline::from_snapshot_result(
        &partial,
        &crate::adapter::TreeOptions::default(),
        None,
    );
    let partial_id = partial
        .save_refmap_with_baseline(&store, Some(&partial_baseline))
        .unwrap();

    let res = execute(args_with_since(Some(&partial_id)), &adapter, &ctx).unwrap();
    assert_eq!(res["since_status"], "incomplete");
    assert!(res.get("tree").is_some());
    assert_ne!(res["snapshot_id"], id1);
}
