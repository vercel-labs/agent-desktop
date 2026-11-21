use std::collections::VecDeque;
use std::sync::Mutex;

use agent_desktop_core::actions::capability;
use agent_desktop_core::context::CommandContext;
use agent_desktop_core::refs::ref_identity::tagged_element_token;
use agent_desktop_core::{
    AccessibilityNode, ActionOps, AdapterError, AppInfo, AppInventory, AppInventoryQuery, Deadline,
    ElementState, ErrorCode, HitTestResult, IdentifierEvidence, InputOps, InteractionLease,
    InteractionPolicy, LiveElement, LiveIdentity, LocatorEvidence, LocatorField,
    LocatorRefEvidence, LocatorStats, NativeHandle, NodeIdentity, NodePresentation,
    NotificationFilter, NotificationInfo, ObservationOps, ObservationRequest, ObservationRoot,
    ObservationSource, ObservedSubtree, ObservedTree, Point, ProcessId, ProcessIdentity, Rect,
    RefCapabilities, RefEntry, RefEntryIdentity, RefGeometry, RefMap, RefProcess, RefScope,
    RefSource, RefStore, SignalBaseline, SignalCompleteness, SignalFilter, SnapshotSurface,
    SystemOps, WindowFilter, WindowInfo, WindowState,
};
use serde_json::json;

#[derive(Default)]
pub(crate) struct WaitStub {
    pub(crate) windows: Mutex<VecDeque<Result<Vec<WindowInfo>, AdapterError>>>,
    pub(crate) tree: Option<AccessibilityNode>,
    pub(crate) menu_times_out: bool,
    pub(crate) element_error: Option<AdapterError>,
}

impl WaitStub {
    pub(crate) fn with_buttons(names: &[&str]) -> Self {
        let mut tree = accessibility_node("window", "Doc");
        tree.children = names
            .iter()
            .map(|name| accessibility_node("button", name))
            .collect();
        Self {
            tree: Some(tree),
            ..Default::default()
        }
    }

    pub(crate) fn unreachable() -> Self {
        Self {
            windows: script(vec![Err(AdapterError::new(
                ErrorCode::AppNotFound,
                "app missing",
            ))]),
            ..Default::default()
        }
    }

    pub(crate) fn rebuilding() -> Self {
        let error = AdapterError::new(ErrorCode::WindowNotFound, "The window list is rebuilding");
        Self {
            windows: script(vec![Err(error), Ok(vec![window("w-1", "Inbox")])]),
            ..Default::default()
        }
    }
}

impl ObservationOps for WaitStub {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let node = self
            .tree
            .clone()
            .unwrap_or_else(|| accessibility_node("window", "Doc"));
        observed_tree(&root, node)
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        next(&self.windows).unwrap_or_else(|| Ok(vec![window("w-1", "Doc")]))
    }

    fn query_app_inventory(
        &self,
        query: AppInventoryQuery<'_>,
        _deadline: Deadline,
    ) -> Result<AppInventory, AdapterError> {
        Ok(query.answer(vec![app()]))
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        if let Some(error) = &self.element_error {
            return Err(error.clone());
        }
        Ok(LiveElement {
            identity: LiveIdentity {
                name: LocatorField::Known("Run".into()),
                description: LocatorField::Absent,
                identifiers: IdentifierEvidence::absent().with_element_token("wait-contract-node"),
            },
            state: ElementState {
                role: "button".into(),
                states: vec![],
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(Rect {
                x: 1.0,
                y: 1.0,
                width: 20.0,
                height: 20.0,
            }),
            available_actions: vec![capability::CLICK.into()],
        })
    }

    fn get_live_state(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        Ok(Some(ElementState {
            role: "button".into(),
            states: vec![],
            value: None,
            enabled: Some(true),
            hidden: Some(false),
            offscreen: Some(false),
        }))
    }

    fn get_live_value(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<String>, AdapterError> {
        Ok(None)
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 1.0,
            y: 1.0,
            width: 20.0,
            height: 20.0,
        }))
    }

    fn get_live_actions(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        Ok(Some(vec![capability::CLICK.into()]))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: Point,
        _deadline: Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::ReachesTarget)
    }
}

impl ActionOps for WaitStub {}
impl InputOps for WaitStub {}

impl SystemOps for WaitStub {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        InteractionLease::guarded(deadline, ())
    }

    fn wait_for_menu(
        &self,
        _process: ProcessIdentity,
        _open: bool,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        match self.menu_times_out {
            true => Err(deadline.timeout_error()),
            false => Ok(()),
        }
    }

    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
        _policy: InteractionPolicy,
        _deadline: Deadline,
        _lease: Option<&InteractionLease>,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        Err(AdapterError::timeout("notification center unavailable"))
    }

    fn capture_signal_baseline(
        &self,
        _filter: &SignalFilter,
        _deadline: Deadline,
    ) -> Result<SignalBaseline, AdapterError> {
        Ok(SignalBaseline {
            windows: vec![],
            apps: vec![],
            surfaces: vec![],
            completeness: SignalCompleteness::complete(),
        })
    }

    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window]
    }
}

pub(crate) fn script<T: Clone>(items: Vec<T>) -> Mutex<VecDeque<T>> {
    Mutex::new(items.into())
}

fn next<T: Clone>(queue: &Mutex<VecDeque<T>>) -> Option<T> {
    let mut queue = queue.lock().unwrap();
    match queue.len() > 1 {
        true => queue.pop_front(),
        false => queue.front().cloned(),
    }
}

pub(crate) fn window(id: &str, title: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: title.into(),
        app: "TextEdit".into(),
        pid: ProcessId::new(42),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: WindowState {
            is_focused: true,
            ..Default::default()
        },
    }
}

fn app() -> AppInfo {
    AppInfo {
        name: "TextEdit".into(),
        pid: ProcessId::new(42),
        bundle_id: None,
        process_instance: Some("test-instance".into()),
        presentation: None,
        aliases: vec![],
    }
}

pub(crate) fn save_run_button(context: &CommandContext) -> (String, String) {
    let bounds = Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    let entry = RefEntry {
        process: RefProcess {
            pid: ProcessId::new(1),
            process_instance: Some("wait-contract-process".into()),
        },
        identity: RefEntryIdentity {
            retained_object: Some(tagged_element_token("wait-contract-node")),
            role: "button".into(),
            name: Some("Run".into()),
            value: None,
            description: None,
            native_id: None,
            unique_at_capture: false,
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: Some(bounds),
            bounds_hash: bounds.bounds_hash(),
        },
        capabilities: RefCapabilities {
            states: vec![],
            available_actions: vec![capability::CLICK.into()],
        },
        source: RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
            renderer_accessibility_forced: false,
        },
        scope: RefScope {
            root_ref: None,
            path_is_absolute: true,
            path: Default::default(),
        },
    };
    let mut refmap = RefMap::new();
    let local = refmap.try_allocate(entry).expect("the entry allocates");
    let store = RefStore::for_session(context.session_id()).expect("the ref store resolves");
    let snapshot_id = store
        .save_new_snapshot(&refmap)
        .expect("the snapshot saves");
    (
        snapshot_id.clone(),
        format!("@{snapshot_id}:{}", local.trim_start_matches('@')),
    )
}

fn accessibility_node(role: &str, name: &str) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: NodeIdentity {
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: NodePresentation::default(),
        children_count: None,
        subtree_truncated: false,
        children: vec![],
    }
}

fn field(value: Option<String>) -> LocatorField<String> {
    value
        .map(LocatorField::Known)
        .unwrap_or(LocatorField::Absent)
}

fn subtree(node: AccessibilityNode) -> ObservedSubtree {
    let native_id = node.identity.native_id.clone();
    let retained = node.identity.retained_object.clone();
    let typed = IdentifierEvidence::typed(native_id, Some(0), true)
        .with_borrowed_name(node.identity.name_is_borrowed);
    let evidence = LocatorEvidence {
        role: LocatorField::Known(node.role),
        name: field(node.identity.name),
        description: field(node.identity.description),
        value: field(node.identity.value),
        identifiers: match retained {
            Some(token) => typed.with_retained_object(token),
            None => typed,
        },
        states: LocatorField::Known(node.presentation.states),
        ref_evidence: LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: node
                .presentation
                .bounds
                .map(LocatorField::Known)
                .unwrap_or(LocatorField::Absent),
            available_actions: LocatorField::Known(node.presentation.available_actions),
            descriptors: node.presentation.descriptors.clone(),
        },
    };
    let children = node.children.into_iter().map(subtree).collect();
    ObservedSubtree::new(
        evidence,
        children,
        !node.subtree_truncated,
        node.children_count,
    )
}

fn observed_tree(
    root: &ObservationRoot<'_>,
    node: AccessibilityNode,
) -> Result<ObservedTree, AdapterError> {
    ObservedTree::from_roots(
        vec![subtree(node)],
        ObservationSource::from_root(root, root.surface()),
        LocatorStats::default(),
        true,
    )
}

pub(crate) fn seed_error() -> AdapterError {
    AdapterError::app_unresponsive("macOS inventory")
        .with_details(json!({"kind": "inventory_sources"}))
}
