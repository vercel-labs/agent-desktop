use crate::adapter::{
    ActionOps, InputOps, NativeHandle, ObservationOps, SnapshotSurface, SystemOps, WindowFilter,
};
use crate::commands::snapshot::{SnapshotArgs, execute};
use crate::context::CommandContext;
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::{
    AccessibilityNode, AdapterError, AppInfo, Deadline, ElementIdentifier, ElementState,
    IdentifierKind, Rect, RefEntry, WindowInfo,
};
use serde_json::Value;

pub(super) struct Stub {
    pub(super) tree: AccessibilityNode,
    pub(super) menu: bool,
    pub(super) state: Option<ElementState>,
    pub(super) bounds: Option<Rect>,
    pub(super) value: Option<String>,
}

impl Stub {
    pub(super) fn showing(tree: AccessibilityNode) -> Self {
        Self {
            tree,
            menu: false,
            state: None,
            bounds: None,
            value: None,
        }
    }
}

fn live<T: Clone>(read: &Option<T>, operation: &str) -> Result<Option<T>, AdapterError> {
    read.clone()
        .map(Some)
        .ok_or_else(|| AdapterError::not_supported(operation))
}

fn cut(mut node: AccessibilityNode, depth: u8, limit: u8) -> AccessibilityNode {
    if depth >= limit && !node.children.is_empty() {
        node.children_count = u32::try_from(node.children.len()).ok();
        node.children.clear();
        return node;
    }
    let children = std::mem::take(&mut node.children);
    node.children = children
        .into_iter()
        .map(|c| cut(c, depth + 1, limit))
        .collect();
    node
}

impl ObservationOps for Stub {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        self.menu && surface == SnapshotSurface::Menu
    }

    fn focused_app(&self, _deadline: Deadline) -> Result<Option<AppInfo>, AdapterError> {
        Ok(Some(AppInfo {
            aliases: Vec::new(),
            name: "FixtureApp".into(),
            pid: crate::ProcessId::new(1),
            bundle_id: None,
            process_instance: Some("test-instance".into()),
            presentation: None,
        }))
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Notes".into(),
            app: "FixtureApp".into(),
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if matches!(root, ObservationRoot::Element { .. }) {
            return crate::adapter::saved_ref_element_tree(root);
        }
        let tree = cut(self.tree.clone(), 0, request.max_logical_depth);
        crate::adapter::observed_tree(&root, tree)
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_state(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        live(&self.state, "get_live_state")
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        live(&self.bounds, "get_element_bounds")
    }

    fn get_live_value(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<String>, AdapterError> {
        live(&self.value, "get_live_value")
    }
}

impl ActionOps for Stub {}
impl InputOps for Stub {}

impl SystemOps for Stub {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        let menu = self.menu.then_some(SnapshotSurface::Menu);
        [SnapshotSurface::Window].into_iter().chain(menu).collect()
    }
}

pub(super) fn node(role: &str, name: &str, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut node = crate::adapter::minimal_accessibility_node(role, name);
    node.children = children;
    node
}

pub(super) fn leaf(role: &str, name: &str) -> AccessibilityNode {
    node(role, name, Vec::new())
}

pub(super) fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

pub(super) fn live_state(states: &[&str]) -> ElementState {
    ElementState {
        role: "checkbox".into(),
        states: states.iter().map(|state| (*state).to_owned()).collect(),
        value: None,
        enabled: Some(true),
        hidden: Some(false),
        offscreen: Some(false),
    }
}

pub(super) fn notes_window() -> AccessibilityNode {
    let mut save = leaf("button", "Save");
    save.presentation.available_actions = vec!["Press".into()];
    save.identity.description = Some("Save the note".into());
    save.presentation.bounds = Some(rect(12.0, 8.0, 64.0, 28.0));
    let mut share = leaf("button", "Share");
    share.presentation.states = vec!["disabled".into()];
    share.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: "share-button".into(),
    });
    let mut title = leaf("textfield", "Title");
    title.identity.value = Some("Meeting notes".into());
    title.presentation.states = vec!["focused".into()];
    title.presentation.descriptors.placeholder = Some("Untitled".into());
    let mut pinned = leaf("checkbox", "Pinned");
    pinned.presentation.states = vec!["checked".into()];
    let label = leaf("statictext", "Last edited today");
    let wrapper = node("group", "", vec![pinned]);
    let panes = node(
        "splitter",
        "Divider",
        vec![leaf("group", "Left"), leaf("group", "Right")],
    );
    let cells = vec![leaf("cell", "Today"), leaf("cell", "Yesterday")];
    let recent = node("group", "Recent", cells);
    let folders = node(
        "scrollarea",
        "Folders",
        vec![node("list", "Library", vec![recent])],
    );
    let toolbar = node("toolbar", "Main toolbar", vec![save, share]);
    let mut window = node(
        "window",
        "Notes",
        vec![toolbar, title, wrapper, label, panes, folders],
    );
    window.presentation.bounds = Some(rect(0.0, 0.0, 640.0, 480.0));
    window
}

pub(super) fn snapshot_args() -> SnapshotArgs {
    SnapshotArgs {
        app: None,
        window_id: None,
        max_depth: 8,
        include_bounds: false,
        interactive_only: false,
        compact: false,
        surface: SnapshotSurface::Window,
        skeleton: false,
        root_ref: None,
        timeout_ms: None,
        force_electron_a11y: false,
    }
}

pub(super) fn snapshot(stub: &Stub, args: SnapshotArgs) -> Value {
    execute(args, stub, &CommandContext::default()).expect("the snapshot succeeds")
}

pub(super) fn ref_named(snapshot: &Value, name: &str) -> String {
    fn walk(node: &Value, name: &str) -> Option<String> {
        if node["name"] == name {
            return node["ref_id"].as_str().map(str::to_owned);
        }
        node["children"]
            .as_array()?
            .iter()
            .find_map(|child| walk(child, name))
    }
    walk(&snapshot["tree"], name).expect("the named node carries a ref")
}

pub(super) fn masked_by_ref(value: Value, ref_id: &str) -> Value {
    let end = ref_id.find(':').expect("a qualified ref") + 1;
    let text = value.to_string().replace(&ref_id[..end], "@<snapshot_id>:");
    serde_json::from_str(&text).expect("masked JSON stays valid")
}

pub(super) fn masked(value: Value) -> Value {
    let prefix = format!(
        "@{}:",
        value["snapshot_id"].as_str().expect("a snapshot_id")
    );
    masked_by_ref(value, &prefix)
}
