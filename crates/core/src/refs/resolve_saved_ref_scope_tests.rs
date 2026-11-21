use std::sync::Mutex;

use super::test_support::{capture_by, node, tree_with, window};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::ObservationRoot;
use crate::refs::refs_test_support::HomeGuard;
use crate::refs::resolve_saved_ref::resolve_saved_ref;
use crate::{
    AccessibilityNode, AdapterError, Deadline, ErrorCode, NativeHandle, RefEntry, WindowInfo,
};

pub(super) struct ScopeAdapter {
    live: Mutex<AccessibilityNode>,
    after_observation: Mutex<Option<AccessibilityNode>>,
    landings: Mutex<Vec<(bool, Vec<usize>)>>,
}

impl ScopeAdapter {
    pub(super) fn new(live: AccessibilityNode) -> Self {
        Self {
            live: Mutex::new(live),
            after_observation: Mutex::new(None),
            landings: Mutex::new(Vec::new()),
        }
    }

    fn changing_after_observation(live: AccessibilityNode, next: AccessibilityNode) -> Self {
        let adapter = Self::new(live);
        *adapter.after_observation.lock().unwrap() = Some(next);
        adapter
    }

    fn node_at(&self, path: &[usize]) -> Option<AccessibilityNode> {
        let mut current = self.live.lock().unwrap().clone();
        for step in path {
            current = current.children.get(*step)?.clone();
        }
        Some(current)
    }
}

impl ObservationOps for ScopeAdapter {
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _deadline: Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Ok(query.answer(vec![crate::AppInfo {
            aliases: Vec::new(),
            name: "Fixture".into(),
            pid: crate::ProcessId::new(1),
            bundle_id: None,
            process_instance: Some("test-instance".into()),
            presentation: None,
        }]))
    }

    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![window()])
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        if let ObservationRoot::Element { handle, .. } = &root {
            let path = handle
                .downcast_ref::<Vec<usize>>()
                .ok_or_else(|| AdapterError::internal("not a landed path"))?;
            let mut landed = self
                .node_at(path)
                .ok_or_else(|| AdapterError::stale_ref_because("landed element is gone"))?;
            landed.children.clear();
            return crate::adapter::observed_tree(&root, landed);
        }
        let observed = crate::adapter::observed_tree(&root, self.live.lock().unwrap().clone());
        if let Some(next) = self.after_observation.lock().unwrap().take() {
            *self.live.lock().unwrap() = next;
        }
        observed
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        base: Option<&NativeHandle>,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        let mut path = match base {
            Some(handle) => handle
                .downcast_ref::<Vec<usize>>()
                .cloned()
                .ok_or_else(|| AdapterError::internal("not a landed path"))?,
            None => Vec::new(),
        };
        path.extend(entry.scope.path.iter().copied());
        self.landings
            .lock()
            .unwrap()
            .push((base.is_some(), path.clone()));
        match self.node_at(&path) {
            Some(_) => Ok(NativeHandle::new(path)),
            None => Err(AdapterError::stale_ref_because("no child at the path")),
        }
    }
}

impl ActionOps for ScopeAdapter {}
impl InputOps for ScopeAdapter {}
impl SystemOps for ScopeAdapter {
    fn supported_surfaces(&self) -> Vec<crate::SnapshotSurface> {
        vec![crate::SnapshotSurface::Window]
    }
}

fn row(name: &str) -> AccessibilityNode {
    let mut row = node("group", name);
    row.children = vec![node("button", "Name"), node("button", "Size")];
    row
}

pub(super) fn rows(names: &[&str]) -> AccessibilityNode {
    tree_with(names.iter().map(|name| row(name)).collect())
}

pub(super) fn name_cell_of_row(root: AccessibilityNode, row: usize) -> RefEntry {
    capture_by(root, |entry| {
        entry.identity.name.as_deref() == Some("Name") && entry.scope.path.first() == Some(&row)
    })
}

pub(super) fn resolve(
    adapter: &ScopeAdapter,
    entry: &RefEntry,
) -> Result<NativeHandle, AdapterError> {
    resolve_saved_ref(adapter, entry, Deadline::standard().unwrap(), None)
}

pub(super) fn landed_path(handle: &NativeHandle) -> Vec<usize> {
    handle
        .downcast_ref::<Vec<usize>>()
        .cloned()
        .expect("a landed path")
}

#[test]
fn a_name_cell_shared_by_every_row_resolves_to_its_own_row() {
    let _guard = HomeGuard::new();
    let tree = rows(&["a.txt", "b.txt", "c.txt"]);
    let entry = name_cell_of_row(tree.clone(), 1);
    let adapter = ScopeAdapter::new(tree);

    let handle = resolve(&adapter, &entry)
        .unwrap_or_else(|error| panic!("{}: {error:?}", "the cell is unique inside its row"));

    assert!(entry.identity.unique_at_capture);
    assert_eq!(entry.scope.chain.len(), 1);
    assert_eq!(entry.scope.chain[0].name.as_deref(), Some("b.txt"));
    assert_eq!(landed_path(&handle), [1, 0]);
    let landings = adapter.landings.lock().unwrap().clone();
    assert_eq!(landings, [(false, vec![1]), (true, vec![1, 0])]);
}

#[test]
fn a_row_inserted_before_the_landing_is_refused_and_never_lands_another_rows_cell() {
    let _guard = HomeGuard::new();
    let before = rows(&["a.txt", "b.txt"]);
    let entry = name_cell_of_row(before.clone(), 1);
    let after = rows(&["new.txt", "a.txt", "b.txt"]);
    let adapter = ScopeAdapter::changing_after_observation(before, after);

    let error = resolve(&adapter, &entry)
        .err()
        .expect("the row the path names changed");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(
        adapter
            .landings
            .lock()
            .unwrap()
            .iter()
            .all(|(_, path)| path.as_slice() != [1, 0]),
        "the cell of another row must never be landed"
    );
}

#[test]
fn a_row_removed_before_the_landing_is_refused() {
    let _guard = HomeGuard::new();
    let before = rows(&["a.txt", "b.txt", "c.txt"]);
    let entry = name_cell_of_row(before.clone(), 2);
    let adapter = ScopeAdapter::changing_after_observation(before, rows(&["a.txt", "c.txt"]));

    let error = resolve(&adapter, &entry)
        .err()
        .expect("the row moved under its path");

    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn a_row_moved_between_observations_resolves_to_its_own_cell() {
    let _guard = HomeGuard::new();
    let before = rows(&["a.txt", "b.txt"]);
    let entry = name_cell_of_row(before.clone(), 1);
    let adapter = ScopeAdapter::new(rows(&["new.txt", "a.txt", "b.txt"]));

    let handle = resolve(&adapter, &entry)
        .unwrap_or_else(|error| panic!("{}: {error:?}", "the row is found again by its proof"));

    assert_eq!(landed_path(&handle), [2, 0]);
}

#[test]
fn a_renamed_scope_ancestor_is_stale() {
    let _guard = HomeGuard::new();
    let before = rows(&["a.txt", "b.txt"]);
    let entry = name_cell_of_row(before.clone(), 1);
    let adapter = ScopeAdapter::new(rows(&["a.txt", "renamed.txt"]));

    let error = resolve(&adapter, &entry)
        .err()
        .expect("the scope no longer exists");

    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn rows_that_are_twins_leave_their_cells_refused() {
    let _guard = HomeGuard::new();
    let tree = rows(&["same.txt", "same.txt"]);
    let entry = name_cell_of_row(tree.clone(), 1);
    let adapter = ScopeAdapter::new(tree);

    let error = resolve(&adapter, &entry)
        .err()
        .expect("twin rows give no scope");

    assert!(!entry.identity.unique_at_capture);
    assert!(entry.scope.chain.is_empty());
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn twins_inside_one_scope_stay_refused() {
    let _guard = HomeGuard::new();
    let mut doubled = node("group", "b.txt");
    doubled.children = vec![node("button", "Name"), node("button", "Name")];
    let tree = tree_with(vec![row("a.txt"), doubled]);
    let entry = capture_by(tree, |entry| {
        entry.identity.name.as_deref() == Some("Name") && entry.scope.path.first() == Some(&1)
    });

    assert!(!entry.identity.unique_at_capture);
    assert!(entry.scope.chain.is_empty());
}

#[test]
fn a_scope_that_needs_its_own_scope_records_the_whole_chain() {
    let _guard = HomeGuard::new();
    let folder = |name: &str| {
        let mut folder = node("group", name);
        folder.children = vec![row("same.txt")];
        folder
    };
    let tree = tree_with(vec![folder("docs"), folder("pics")]);
    let entry = capture_by(tree.clone(), |entry| {
        entry.identity.name.as_deref() == Some("Name") && entry.scope.path.first() == Some(&1)
    });
    let adapter = ScopeAdapter::new(tree);

    let handle = resolve(&adapter, &entry)
        .unwrap_or_else(|error| panic!("{}: {error:?}", "the chain names one cell"));

    let names: Vec<_> = entry
        .scope
        .chain
        .iter()
        .map(|identity| identity.name.as_deref())
        .collect();
    assert_eq!(names, [Some("same.txt"), Some("pics")]);
    assert_eq!(landed_path(&handle), [1, 0, 0]);
    let landings = adapter.landings.lock().unwrap().clone();
    assert_eq!(
        landings,
        [(false, vec![1]), (true, vec![1, 0]), (true, vec![1, 0, 0])]
    );
}

#[test]
fn a_nested_twin_is_scoped_by_its_inner_parent_and_the_outer_one_is_refused() {
    let _guard = HomeGuard::new();
    let mut inner = node("treeitem", "Desktop");
    inner.children = Vec::new();
    let mut this_pc = node("treeitem", "This PC");
    this_pc.children = vec![inner];
    let mut outer = node("treeitem", "Desktop");
    outer.children = vec![this_pc];
    let tree = tree_with(vec![outer]);
    let by_path = |path: &'static [usize]| {
        capture_by(tree.clone(), move |entry| {
            entry.identity.name.as_deref() == Some("Desktop") && entry.scope.path.as_slice() == path
        })
    };

    assert!(by_path(&[0, 0, 0]).identity.unique_at_capture);
    assert!(!by_path(&[0]).identity.unique_at_capture);
}

#[test]
fn a_refmap_written_before_scope_chains_loads_without_one() {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    entry.scope.chain.clear();
    let mut written = serde_json::to_value(&entry).unwrap();
    assert!(written["chain"].is_null(), "an empty chain is not written");
    written.as_object_mut().unwrap().remove("chain");

    let loaded: RefEntry = serde_json::from_value(written).unwrap();

    assert!(loaded.scope.chain.is_empty());
}
