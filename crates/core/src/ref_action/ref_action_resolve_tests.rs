use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::adapter::{ActionOps, InputOps, LiveElement, ObservationOps, SystemOps};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::ref_action::execute_entry_with_context;
use crate::{
    AccessibilityNode, Action, ActionRequest, ActionResult, AdapterError, CommandContext,
    DeliverySemantics, ErrorCode, IdentityProof, InteractionLease, NativeHandle, Rect, RefEntry,
    RefEntryIdentity, ScopeChain,
};

struct RecycledRowAdapter {
    row_recycled: AtomicBool,
    dispatches: AtomicU32,
}

impl RecycledRowAdapter {
    fn delete_button() -> AccessibilityNode {
        let mut button = crate::adapter::minimal_accessibility_node("button", "Delete");
        button.presentation.available_actions = vec!["Click".into()];
        button.presentation.bounds = Some(Rect {
            x: 10.0,
            y: 10.0,
            width: 40.0,
            height: 20.0,
        });
        button
    }

    fn row_node(&self) -> AccessibilityNode {
        let row_name = if self.row_recycled.load(Ordering::SeqCst) {
            "Account B"
        } else {
            "Account A"
        };
        let mut row = crate::adapter::minimal_accessibility_node("row", row_name);
        row.children = vec![Self::delete_button()];
        row
    }

    fn window_node(&self) -> AccessibilityNode {
        let mut window = crate::adapter::minimal_accessibility_node("window", "Accounts");
        window.children = vec![self.row_node()];
        window
    }
}

impl ObservationOps for RecycledRowAdapter {
    crate::adapter::saved_ref_scope!(@inventory);
    crate::adapter::saved_ref_anchor!();

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        match &root {
            ObservationRoot::Element { entry, .. } if entry.identity.role == "row" => {
                crate::adapter::observed_tree(&root, self.row_node())
            }
            ObservationRoot::Element { .. } => {
                crate::adapter::observed_tree(&root, Self::delete_button())
            }
            _ => crate::adapter::observed_tree(&root, self.window_node()),
        }
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<LiveElement, AdapterError> {
        Ok(LiveElement {
            identity: crate::adapter::live_identity("Delete"),
            state: crate::tree::element_state::ElementState {
                role: "button".into(),
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(Rect {
                x: 10.0,
                y: 10.0,
                width: 40.0,
                height: 20.0,
            }),
            available_actions: vec!["Click".into()],
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 10.0,
            y: 10.0,
            width: 40.0,
            height: 20.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<crate::adapter::hit_test::HitTestResult, AdapterError> {
        Ok(crate::adapter::hit_test::HitTestResult::ReachesTarget)
    }

    fn list_surfaces(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::SurfaceInfo>, AdapterError> {
        Ok(Vec::new())
    }
}

impl ActionOps for RecycledRowAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for RecycledRowAdapter {}

impl SystemOps for RecycledRowAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        self.row_recycled.store(true, Ordering::SeqCst);
        InteractionLease::guarded(deadline, ())
    }
}

fn scoped_delete_entry() -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Delete"));
    entry.identity.retained_object = None;
    entry.identity.proof = IdentityProof::Unique;
    entry.source.source_app = Some("Test".into());
    entry.source.source_window_id = Some("w-1".into());
    entry.capabilities.available_actions = vec!["Click".into()];
    entry.scope.chain = ScopeChain::from_shared(vec![Arc::new(RefEntryIdentity {
        role: "row".into(),
        name: Some("Account A".into()),
        proof: IdentityProof::Unique,
        ..RefEntryIdentity::default()
    })]);
    entry
}

#[test]
fn queued_scoped_ref_revalidates_ancestors_under_lease_and_refuses_recycled_row() {
    let adapter = RecycledRowAdapter {
        row_recycled: AtomicBool::new(false),
        dispatches: AtomicU32::new(0),
    };
    let entry = scoped_delete_entry();
    let request = ActionRequest::headless(Action::Click);
    let error = execute_entry_with_context(&adapter, &entry, request, &CommandContext::default())
        .expect_err("a recycled ancestor row must not be acted on through a cached child handle");

    assert!(adapter.row_recycled.load(Ordering::SeqCst));
    assert_eq!(error.code, ErrorCode::StaleRef, "{error:?}");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
}
