//! The fake desktop the menu fixtures drive.
//!
//! It serves one application with one window and one row, answers each menu
//! read from a script, and reports a caller-chosen delivery outcome. Everything
//! else — ref resolution, the action, the interaction lease — behaves like any
//! other adapter so the tests can start at the real commands.

use std::collections::VecDeque;
use std::sync::Mutex;

use super::*;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter};
use crate::locator::live_locator::{
    IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence, LocatorStats,
    ObservationRequest, ObservationRoot, ObservationSource, ObservedSubtree, ObservedTree,
};
use crate::refs::refs_store::RefStore;
use crate::{
    ActionRequest, ActionResult, AdapterError, AppInfo, ErrorCode, ProcessId, Rect, RefEntry,
    SnapshotSurface, WindowInfo, commands,
};

pub(crate) const APP: &str = "Fixture";
pub(crate) const PID: ProcessId = ProcessId::new(42);
pub(crate) const GENERATION: &str = "generation-1";
pub(crate) const TOKEN_A: &str = "axtok1:aa";
pub(crate) const TOKEN_B: &str = "axtok1:bb";

/// One scripted answer to a menu read.
#[derive(Clone, Copy)]
pub(crate) enum MenuAnswer {
    Absent,
    Named(&'static str),
    Unnamed,
    Unreadable,
}

pub(crate) struct FixtureAdapter {
    reads: Mutex<VecDeque<MenuAnswer>>,
    delivered: bool,
}

impl FixtureAdapter {
    pub(crate) fn new(reads: &[MenuAnswer], delivered: bool) -> Self {
        Self {
            reads: Mutex::new(reads.iter().copied().collect()),
            delivered,
        }
    }

    fn app() -> AppInfo {
        AppInfo {
            name: APP.into(),
            pid: PID,
            bundle_id: None,
            process_instance: Some(GENERATION.into()),
            presentation: None,
        }
    }
}

fn item(name: &str) -> ObservedSubtree {
    ObservedSubtree::new(
        LocatorEvidence {
            role: LocatorField::Known("menuitem".into()),
            name: LocatorField::Known(name.into()),
            description: LocatorField::Absent,
            value: LocatorField::Absent,
            identifiers: IdentifierEvidence::absent(),
            states: LocatorField::Known(Vec::new()),
            ref_evidence: LocatorRefEvidence {
                bounds: LocatorField::Known(Rect {
                    x: 100.0,
                    y: 200.0,
                    width: 240.0,
                    height: 24.0,
                }),
                window_bounds: LocatorField::Absent,
                available_actions: LocatorField::Known(Vec::new()),
                descriptors: Default::default(),
            },
        },
        Vec::new(),
        true,
        None,
    )
}

fn menu_tree(app: &AppInfo, token: Option<&str>) -> ObservedTree {
    let mut identifiers = IdentifierEvidence::absent();
    if let Some(token) = token {
        identifiers = identifiers.with_retained_object(token.into());
    }
    let root = ObservedSubtree::new(
        LocatorEvidence {
            role: LocatorField::Known("menu".into()),
            name: LocatorField::Known("context".into()),
            description: LocatorField::Absent,
            value: LocatorField::Absent,
            identifiers,
            states: LocatorField::Known(Vec::new()),
            ref_evidence: LocatorRefEvidence {
                bounds: LocatorField::Known(Rect {
                    x: 100.0,
                    y: 200.0,
                    width: 240.0,
                    height: 48.0,
                }),
                window_bounds: LocatorField::Absent,
                available_actions: LocatorField::Known(Vec::new()),
                descriptors: Default::default(),
            },
        },
        vec![item("Rename")],
        true,
        None,
    );
    ObservedTree::from_roots(
        vec![root],
        ObservationSource::Application {
            app: app.clone(),
            surface: SnapshotSurface::Menu,
        },
        LocatorStats::default(),
        true,
    )
    .expect("the fixture menu is observable")
}

impl ObservationOps for FixtureAdapter {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        surface == SnapshotSurface::Menu
    }

    fn retains_element_tokens(&self) -> bool {
        true
    }

    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![Self::app()])
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Fixture".into(),
            app: APP.into(),
            pid: PID,
            process_instance: Some(GENERATION.into()),
            bounds: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            }),
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let ObservationRoot::Application { app, .. } = root else {
            return crate::adapter::saved_ref_tree(root, "button", Some("Open"));
        };
        let answer = self
            .reads
            .lock()
            .ok()
            .and_then(|mut reads| reads.pop_front())
            .unwrap_or(MenuAnswer::Absent);
        match answer {
            MenuAnswer::Absent => Err(AdapterError::surface_absent(
                SnapshotSurface::Menu,
                "No menu is open",
            )),
            MenuAnswer::Named(token) => Ok(menu_tree(app, Some(token))),
            MenuAnswer::Unnamed => Ok(menu_tree(app, None)),
            MenuAnswer::Unreadable => Err(AdapterError::new(
                ErrorCode::Timeout,
                "The menu could not be read",
            )),
        }
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_element_strict(entry, deadline)
    }

    crate::adapter::complete_live_observation!(
        "button",
        "Open",
        [crate::actions::capability::RIGHT_CLICK]
    );
}

impl ActionOps for FixtureAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        if self.delivered {
            Ok(ActionResult::delivered_unverified("right_click"))
        } else {
            Ok(ActionResult::satisfied_without_delivery("right_click"))
        }
    }
}

impl InputOps for FixtureAdapter {}

impl SystemOps for FixtureAdapter {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Menu]
    }

    crate::adapter::guarded_interaction_lease!();
}

/// Saves the row this fixture acts on and answers the ref that names it.
pub(crate) fn row_ref() -> String {
    let mut refmap = crate::refs::RefMap::new();
    let bounds = Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    refmap.allocate(RefEntry {
        process: crate::RefProcess {
            pid: PID,
            process_instance: Some(GENERATION.into()),
        },
        identity: crate::RefEntryIdentity {
            retained_object: None,
            role: "button".into(),
            name: Some("Open".into()),
            value: None,
            description: None,
            native_id: None,
            unique_at_capture: true,
        },
        geometry: crate::RefGeometry {
            bounds_space: None,
            bounds: Some(bounds),
            bounds_hash: bounds.bounds_hash(),
        },
        capabilities: crate::RefCapabilities {
            states: Vec::new(),
            available_actions: vec!["RightClick".into()],
        },
        source: crate::RefSource {
            source_app: Some(APP.into()),
            source_window_id: Some("w-1".into()),
            source_window_title: Some("Fixture".into()),
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
            renderer_accessibility_forced: false,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
    });
    let snapshot = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    format!("@{snapshot}:e1")
}

pub(crate) fn right_click(ref_id: &str, reads: &[MenuAnswer], delivered: bool) -> Value {
    commands::right_click::execute(
        crate::commands::helpers::RefArgs {
            ref_id: ref_id.to_owned(),
            timeout_ms: None,
        },
        &FixtureAdapter::new(reads, delivered),
        &CommandContext::default(),
    )
    .expect("a right click reports its own delivery")
}

pub(crate) fn menu_snapshot(reads: &[MenuAnswer]) -> Value {
    commands::snapshot::execute(
        commands::snapshot::SnapshotArgs {
            app: Some(APP.into()),
            window_id: None,
            max_depth: 20,
            include_bounds: true,
            interactive_only: false,
            compact: false,
            surface: SnapshotSurface::Menu,
            skeleton: false,
            root_ref: None,
            timeout_ms: None,
            force_electron_a11y: false,
        },
        &FixtureAdapter::new(reads, true),
        &CommandContext::default(),
    )
    .expect("a menu surface is readable")
}

pub(crate) fn instance_of(token: &str) -> String {
    instance::of_tree(&crate::AccessibilityNode {
        ref_id: None,
        role: "menu".into(),
        identity: crate::NodeIdentity {
            retained_object: Some(token.into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: Vec::new(),
    })
    .expect("the fixture token names a menu")
    .id()
    .to_owned()
}
