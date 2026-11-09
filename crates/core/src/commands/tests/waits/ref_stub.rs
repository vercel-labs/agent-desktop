use super::stub::build;
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::refs::{RefMap, refs_store::RefStore};
use crate::system::process_state::ProcessState;
use crate::{adapter::*, *};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub(super) struct RefStub {
    pub(super) disabled: bool,
    pub(super) element_error: Option<AdapterError>,
    pub(super) closes_window: bool,
    pub(super) process_exited: bool,
    pub(super) tree_error: Option<AdapterError>,
    pub(super) dispatched: AtomicBool,
}

impl RefStub {
    fn after(&self, effect: bool) -> bool {
        effect && self.dispatched.load(Ordering::SeqCst)
    }
}

type Handle = Result<NativeHandle, AdapterError>;

impl ObservationOps for RefStub {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        match &self.tree_error {
            Some(error) if self.after(true) => Err(error.clone()),
            _ => saved_ref_tree(root, "button", Some("Run")),
        }
    }

    fn resolve_element_strict(&self, _: &RefEntry, _: Deadline) -> Handle {
        Ok(NativeHandle::null())
    }

    fn list_windows(&self, _: &WindowFilter, _: Deadline) -> Result<Vec<WindowInfo>, AdapterError> {
        let mut windows = saved_ref_window_pool();
        windows.retain(|window| !(self.after(self.closes_window) && window.id == "w-1"));
        Ok(windows)
    }

    fn get_live_element(&self, _: &NativeHandle, _: Deadline) -> Result<LiveElement, AdapterError> {
        if let Some(error) = &self.element_error {
            return Err(error.clone());
        }
        Ok(LiveElement {
            identity: live_identity("Run"),
            state: build(json!({"role": "button", "enabled": !self.disabled,
                "hidden": false, "offscreen": false})),
            states_complete: true,
            bounds: Some(build(
                json!({"x": 1.0, "y": 1.0, "width": 20.0, "height": 20.0}),
            )),
            available_actions: vec![crate::actions::capability::CLICK.into()],
        })
    }

    saved_ref_anchor!();
}

impl ActionOps for RefStub {
    fn execute_action(
        &self,
        _: &NativeHandle,
        _: ActionRequest,
        _: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatched.store(true, Ordering::SeqCst);
        let error =
            AdapterError::app_unresponsive("TargetApp").with_details(json!({"ax_error": -25204}));
        Err(error.with_disposition(DeliverySemantics::uncertain()))
    }
}

impl InputOps for RefStub {}

impl SystemOps for RefStub {
    guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        window: &WindowInfo,
        _: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        match self.after(self.closes_window) {
            true => Err(AdapterError::new(
                ErrorCode::WindowNotFound,
                "Window closed",
            )),
            false => Ok(window.clone()),
        }
    }

    fn process_state(&self, _: ProcessIdentity, _: Deadline) -> Result<ProcessState, AdapterError> {
        match self.after(self.process_exited) {
            true => Ok(ProcessState::Exited { code: Some(1) }),
            false => Ok(ProcessState::Running),
        }
    }
}

pub(super) fn save_run_button(window_id: Option<&str>) -> String {
    let mut entry = minimal_ref_entry("button", Some("Run"));
    entry.capabilities.available_actions = vec!["Click".into()];
    entry.source.source_window_id = window_id.map(str::to_string);
    let mut refmap = RefMap::new();
    refmap.allocate(entry);
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}
