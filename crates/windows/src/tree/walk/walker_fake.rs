use agent_desktop_core::{
    AdapterError, Deadline, LocatorEvidence, ObservationRoot, ProcessId, RefEntry, WindowInfo,
    WindowState,
};
use std::collections::{HashMap, HashSet};

use crate::tree::automation::{ERR_NONE, UiaFailure};
use crate::tree::name_evidence::LabelOutcome;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::properties::{ElementProperties, PropertyOutcome};
use crate::tree::walk::walker::{
    NodeKey, TreeSource, WalkBudget, WalkOutcome, walk_from_root, walk_vocabulary,
};

/// The benign end-of-list pair A14-3 measured: `code() == 0` with `result()`
/// `None`, on both build 17763 and build 26100.
pub(crate) const EXHAUSTION: UiaFailure = UiaFailure::Sentinel(ERR_NONE);

/// The real enumeration fault A14-4 measured: descent against a provider whose
/// process has gone away returns `E_FAIL` with `result()` `Some`.
pub(crate) use crate::system::runtime::hresult::E_FAIL;
pub(crate) const REAL_FAILURE: UiaFailure = UiaFailure::Hresult(E_FAIL);

/// The nodes on which each enumeration step should fail, or answer slowly,
/// rather than answer promptly, grouped so `FakeTree` itself stays under the
/// struct field cap.
#[derive(Default)]
struct EnumerationFaults {
    first_child: HashSet<i32>,
    next_sibling: HashSet<i32>,
    first_child_delay: HashMap<i32, std::time::Duration>,
}

/// An in-memory enumerator that answers on the same trait the live UI
/// Automation walker implements, so the correctness branches these tests drive
/// are the branches that ship.
#[derive(Default)]
pub(crate) struct FakeTree {
    first: HashMap<i32, i32>,
    next: HashMap<i32, i32>,
    alias: HashMap<i32, i32>,
    unkeyed: HashSet<i32>,
    wrappers: HashSet<i32>,
    faults: EnumerationFaults,
    reads: HashMap<i32, Vec<(TreeProperty, PropertyOutcome)>>,
}

impl FakeTree {
    pub(crate) fn with_children(mut self, parent: i32, children: &[i32]) -> Self {
        if let Some(first) = children.first() {
            self.first.insert(parent, *first);
        }
        for pair in children.windows(2) {
            self.next.insert(pair[0], pair[1]);
        }
        self
    }

    pub(crate) fn with_chain(self, chain: &[i32]) -> Self {
        chain
            .windows(2)
            .fold(self, |tree, pair| tree.with_children(pair[0], &pair[1..]))
    }

    /// Makes one node report the identity of another, which is what a cycle
    /// looks like from the client side.
    pub(crate) fn aliasing(mut self, node: i32, to: i32) -> Self {
        self.alias.insert(node, to);
        self
    }

    /// Makes a node publish no runtime id, forcing the guard onto its
    /// element-comparison fallback.
    pub(crate) fn unkeyed(mut self, node: i32) -> Self {
        self.unkeyed.insert(node);
        self
    }

    pub(crate) fn wrapping(mut self, node: i32) -> Self {
        self.wrappers.insert(node);
        self
    }

    /// Gives one node a read set, so a vocabulary assertion can be driven end
    /// to end through the real evidence path rather than by calling the
    /// producer directly.
    ///
    /// Without this the fake answered every node with an empty read set, which
    /// is why `states` could not be asserted through the walk at all.
    pub(crate) fn reading(mut self, node: i32, reads: &[(TreeProperty, PropertyOutcome)]) -> Self {
        self.reads.insert(node, reads.to_vec());
        self
    }

    pub(crate) fn faulting_on_first_child(mut self, node: i32) -> Self {
        self.faults.first_child.insert(node);
        self
    }

    pub(crate) fn faulting_on_next_sibling(mut self, node: i32) -> Self {
        self.faults.next_sibling.insert(node);
        self
    }

    /// Makes `first_child` block for `delay` before answering, so a walk
    /// bound by a short deadline observes it as expired.
    pub(crate) fn slow_first_child(mut self, node: i32, delay: std::time::Duration) -> Self {
        self.faults.first_child_delay.insert(node, delay);
        self
    }

    /// A sibling list that never ends. The ancestor guard cannot see it: no
    /// element repeats on any root-to-node path.
    pub(crate) fn looping_siblings(mut self, node: i32) -> Self {
        self.next.insert(node, node);
        self
    }

    fn alias_of(&self, node: i32) -> i32 {
        self.alias.get(&node).copied().unwrap_or(node)
    }
}

impl TreeSource for FakeTree {
    type Node = i32;

    fn first_child(&self, node: &i32) -> Result<i32, UiaFailure> {
        if let Some(delay) = self.faults.first_child_delay.get(node) {
            std::thread::sleep(*delay);
        }
        if self.faults.first_child.contains(node) {
            return Err(REAL_FAILURE);
        }
        self.first.get(node).copied().ok_or(EXHAUSTION)
    }

    fn next_sibling(&self, node: &i32) -> Result<i32, UiaFailure> {
        if self.faults.next_sibling.contains(node) {
            return Err(REAL_FAILURE);
        }
        self.next.get(node).copied().ok_or(EXHAUSTION)
    }

    fn identity(&self, node: &i32) -> NodeKey {
        if self.unkeyed.contains(node) {
            return NodeKey::Unavailable;
        }
        NodeKey::Runtime(vec![self.alias_of(*node)])
    }

    fn same_element(&self, left: &i32, right: &i32) -> bool {
        self.alias_of(*left) == self.alias_of(*right)
    }

    fn evidence(&self, node: &i32) -> (ElementProperties, LocatorEvidence, u64) {
        let properties =
            ElementProperties::from_reads(self.reads.get(node).cloned().unwrap_or_default());
        let vocabulary = walk_vocabulary(&properties, &LabelOutcome::Unlabelled);
        (
            properties.clone(),
            properties.locator_evidence(vocabulary),
            0,
        )
    }

    fn is_web_wrapper(&self, node: &i32, _properties: &ElementProperties) -> bool {
        self.wrappers.contains(node)
    }
}

pub(crate) fn deadline() -> Deadline {
    Deadline::standard().expect("a standard deadline")
}

/// A minimal, valid `RefEntry` for a control of the given role, with every
/// other field at its blank default so a resolver test overrides only the
/// handful of fields its case actually needs.
pub(crate) fn ref_entry(role: &str) -> RefEntry {
    RefEntry {
        process: agent_desktop_core::RefProcess {
            pid: ProcessId::new(1),
            process_instance: None,
        },
        identity: agent_desktop_core::RefEntryIdentity {
            retained_object: None,
            role: role.to_string(),
            name: None,
            value: None,
            description: None,
            native_id: None,
            unique_at_capture: false,
        },
        geometry: agent_desktop_core::RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: agent_desktop_core::RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: agent_desktop_core::RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: agent_desktop_core::SnapshotSurface::Window,
            renderer_accessibility_forced: false,
        },
        scope: agent_desktop_core::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: agent_desktop_core::refs::RefPath::default(),
        },
    }
}

pub(crate) fn budget(max_logical_depth: u8) -> WalkBudget {
    WalkBudget::new(max_logical_depth, deadline())
}

pub(crate) fn window() -> WindowInfo {
    WindowInfo {
        id: String::from("w-fake"),
        title: String::new(),
        app: String::new(),
        pid: ProcessId::new(0),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    }
}

pub(crate) fn walk(fake: &FakeTree, budget: WalkBudget) -> WalkOutcome {
    let window = window();
    walk_from_root(fake, &1, &ObservationRoot::Window(&window), budget)
        .expect("the walk assembles an observation")
}

/// Runs a walk that is expected not to assemble an observation at all, so the
/// error the caller receives can be asserted.
pub(crate) fn walk_expecting_failure(fake: &FakeTree, budget: WalkBudget) -> AdapterError {
    let window = window();
    walk_from_root(fake, &1, &ObservationRoot::Window(&window), budget)
        .err()
        .expect("the walk was expected to fail")
}
