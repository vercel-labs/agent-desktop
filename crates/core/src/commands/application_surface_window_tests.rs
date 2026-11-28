use super::application_surface_tests::MenuAdapter;
use crate::{Deadline, RefEntry, SnapshotOwner, SnapshotSurface, WindowInfo};
fn popover_owner_surface_window() -> WindowInfo {
    WindowInfo {
        id: "w-77".into(),
        title: "Filters".into(),
        app: "MenuApp".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("instance-42".into()),
        bounds: None,
        state: Default::default(),
    }
}

fn popover_owners(adapter: &MenuAdapter) -> Vec<SnapshotOwner> {
    crate::snapshot::surface_target::resolve_owners(
        adapter,
        Some("MenuApp"),
        None,
        SnapshotSurface::Popover,
        Deadline::standard().unwrap(),
    )
    .unwrap()
}

#[test]
fn a_surface_the_adapter_presents_in_its_own_window_is_owned_by_that_window() {
    let mut adapter = MenuAdapter::windowless();
    adapter.surface_window = Some(popover_owner_surface_window());

    let owners = popover_owners(&adapter);

    assert!(
        matches!(&owners[..], [SnapshotOwner::Window(window)] if window.id == "w-77"),
        "a surface presented by a window is captured at that window: {owners:?}"
    );
}

#[test]
fn a_surface_with_no_window_of_its_own_stays_application_owned() {
    let owners = popover_owners(&MenuAdapter::windowless());

    assert!(
        matches!(&owners[..], [SnapshotOwner::Application(_)]),
        "a surface with no window stays application-owned: {owners:?}"
    );
}

fn popover_entry(window_id: &str) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.process.pid = crate::ProcessId::new(42);
    entry.process.process_instance = Some("instance-42".into());
    entry.source.source_app = Some("MenuApp".into());
    entry.source.source_surface = SnapshotSurface::Popover;
    entry.source.source_window_id = Some(window_id.into());
    entry.source.source_window_title = Some("Filters".into());
    entry
}

#[test]
fn a_recorded_popover_reference_resolves_to_the_popover_window_it_named() {
    let mut adapter = MenuAdapter::windowless();
    adapter.surface_window = Some(popover_owner_surface_window());

    let window = crate::refs::resolve_saved_ref::recorded_window(
        &adapter,
        &popover_entry("w-77"),
        Deadline::standard().unwrap(),
    )
    .unwrap();

    assert_eq!(window.id, "w-77");
    assert_eq!(window.pid, crate::ProcessId::new(42));
    assert_eq!(window.process_instance.as_deref(), Some("instance-42"));
}

#[test]
fn a_recorded_popover_reference_to_a_closed_popover_is_stale() {
    let mut adapter = MenuAdapter::windowless();
    adapter.surface_window = Some(popover_owner_surface_window());

    let error = crate::refs::resolve_saved_ref::recorded_window(
        &adapter,
        &popover_entry("w-99"),
        Deadline::standard().unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code.as_str(), "STALE_REF");
}

#[test]
fn naming_the_surface_window_reads_it_even_though_it_is_not_a_listed_window() {
    let mut adapter = MenuAdapter::windowless();
    adapter.surface_window = Some(popover_owner_surface_window());

    let owners = crate::snapshot::surface_target::resolve_owners(
        &adapter,
        Some("MenuApp"),
        Some("w-77"),
        SnapshotSurface::Popover,
        Deadline::standard().unwrap(),
    )
    .unwrap();

    assert!(
        matches!(&owners[..], [SnapshotOwner::Window(window)] if window.id == "w-77"),
        "the named surface window is captured at that window: {owners:?}"
    );
}

/// An adapter whose popover surface behaves the way Windows does: the surface is
/// application-owned, two popovers are open, and an observation that starts at
/// the application root names neither, so it is refused as ambiguous. Only the
/// id a ref recorded can pick one popover out of the two.
#[derive(Default)]
struct TwoPopoverAdapter {
    observed: std::sync::Mutex<Vec<(String, SnapshotSurface)>>,
}

impl TwoPopoverAdapter {
    fn popover(id: &str) -> WindowInfo {
        WindowInfo {
            id: id.into(),
            ..popover_owner_surface_window()
        }
    }

    fn observed_roots(&self) -> Vec<(String, SnapshotSurface)> {
        self.observed.lock().unwrap().clone()
    }
}

impl crate::adapter::ObservationOps for TwoPopoverAdapter {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        surface == SnapshotSurface::Popover
    }

    fn recorded_surface_window(
        &self,
        process: crate::ProcessIdentity,
        window_id: &str,
        surface: SnapshotSurface,
        _deadline: Deadline,
    ) -> Result<Option<WindowInfo>, crate::AdapterError> {
        let open = ["w-1", "w-2"];
        Ok(open
            .contains(&window_id)
            .then(|| Self::popover(window_id))
            .filter(|window| {
                surface == SnapshotSurface::Popover
                    && window.pid == process.pid
                    && window.process_instance.as_deref() == Some(process.instance.as_str())
            }))
    }

    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, crate::AdapterError> {
        Ok(Vec::new())
    }

    fn observe_tree(
        &self,
        root: crate::ObservationRoot<'_>,
        request: &crate::ObservationRequest,
    ) -> Result<crate::ObservedTree, crate::AdapterError> {
        match root {
            crate::ObservationRoot::Application { .. }
                if request.surface == SnapshotSurface::Popover =>
            {
                Err(crate::AdapterError::new(
                    crate::ErrorCode::AmbiguousTarget,
                    "More than one popover is open in this application: w-1, w-2",
                ))
            }
            crate::ObservationRoot::Window(window) => {
                self.observed
                    .lock()
                    .unwrap()
                    .push((window.id.clone(), request.surface));
                crate::adapter::observed_tree(&root, popover_node())
            }
            crate::ObservationRoot::Application { .. } | crate::ObservationRoot::Element { .. } => {
                crate::adapter::observed_tree(&root, popover_node())
            }
        }
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &RefEntry,
        _base: Option<&crate::NativeHandle>,
        _deadline: Deadline,
    ) -> Result<crate::NativeHandle, crate::AdapterError> {
        Ok(crate::NativeHandle::new("popover-anchor"))
    }
}

impl crate::adapter::ActionOps for TwoPopoverAdapter {}
impl crate::adapter::InputOps for TwoPopoverAdapter {}
impl crate::adapter::SystemOps for TwoPopoverAdapter {}

fn popover_node() -> crate::AccessibilityNode {
    crate::adapter::minimal_accessibility_node("button", "Run")
}

#[test]
fn a_saved_popover_ref_is_observed_at_the_window_it_recorded() {
    let adapter = TwoPopoverAdapter::default();
    let entry = popover_entry("w-1");

    let (window, _, _) = crate::refs::resolve_saved_ref::observe_saved_ref(
        &adapter,
        &entry,
        Deadline::standard().unwrap(),
        None,
        true,
    )
    .expect("the recorded popover answers for its own ref while another popover is open");

    assert_eq!(window.id, "w-1");
    assert_eq!(
        adapter.observed_roots(),
        vec![("w-1".to_string(), SnapshotSurface::Popover)],
        "the observation names the popover the ref recorded"
    );
}

#[test]
fn a_saved_popover_ref_resolves_while_a_second_popover_is_open() {
    let adapter = TwoPopoverAdapter::default();
    let entry = popover_entry("w-2");

    let handle = crate::refs::resolve_saved_ref::resolve_saved_ref(
        &adapter,
        &entry,
        Deadline::standard().unwrap(),
        None,
    )
    .expect("a saved ref naming its popover is never ambiguous");

    assert_eq!(
        handle.downcast_ref::<&'static str>().copied(),
        Some("popover-anchor")
    );
    assert_eq!(
        adapter.observed_roots().first().cloned(),
        Some(("w-2".to_string(), SnapshotSurface::Popover)),
        "resolution reads the popover the ref recorded"
    );
}
