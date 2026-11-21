use super::super::golden::assert_golden;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::click;
use crate::commands::helpers::RefArgs;
use crate::context::{CommandContext, WaitSelector};
use crate::refs::refs_store::RefStore;
use crate::refs::refs_test_support::HomeGuard;
use crate::refs::{RefEntry, RefMap};
use crate::system::process_state::ProcessState;
use crate::{
    Action, ActionRequest, ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics,
    ErrorPayload, InteractionLease, ProcessIdentity, SnapshotSurface, StepMechanism,
    actions::capability,
};
use serde_json::{Value, json};

struct ClickStub {
    uncertain: bool,
}

impl ObservationOps for ClickStub {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        surface == SnapshotSurface::Menubar
    }

    fn resolve_element_strict(
        &self,
        _: &RefEntry,
        _: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!("checkbox", "Agree", [capability::CLICK]);
    crate::adapter::saved_ref_scope!("checkbox", Some("Agree"));
}

impl ActionOps for ClickStub {
    fn execute_action(
        &self,
        _: &NativeHandle,
        _: ActionRequest,
        _: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        if self.uncertain {
            return Err(AdapterError::app_unresponsive("TargetApp")
                .with_details(json!({ "ax_error": -25204 }))
                .with_disposition(DeliverySemantics::uncertain()));
        }
        let press = ActionStep::succeeded("press")
            .with_mechanism(StepMechanism::SemanticApi)
            .with_verified(true);
        let checked = json!({
            "role": "checkbox",
            "states": ["checked"],
            "enabled": true,
            "hidden": false,
            "offscreen": false
        });
        let checked = serde_json::from_value(checked).unwrap();
        Ok(ActionResult::from_execution(&Action::Click, vec![press]).with_state(checked))
    }
}

impl InputOps for ClickStub {}

impl SystemOps for ClickStub {
    crate::adapter::guarded_interaction_lease!();

    fn process_state(&self, _: ProcessIdentity, _: Deadline) -> Result<ProcessState, AdapterError> {
        Ok(ProcessState::Running)
    }
}

fn click_ref(uncertain: bool, context: &CommandContext) -> Result<Value, crate::AppError> {
    let mut entry = crate::adapter::minimal_ref_entry("checkbox", Some("Agree"));
    entry.capabilities.available_actions = vec![capability::CLICK.into()];
    entry.source.source_app = Some("TargetApp".into());
    entry.source.source_surface = SnapshotSurface::Menubar;
    let mut refmap = RefMap::new();
    refmap.allocate(entry);
    let snapshot = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let args = RefArgs {
        ref_id: format!("@{snapshot}:e1"),
        timeout_ms: None,
    };
    click::execute(args, &ClickStub { uncertain }, context)
}

#[test]
fn click_reports_the_state_change_and_an_uncertain_click_carries_the_post_action_observation() {
    let _home = HomeGuard::new();
    let wait = WaitSelector {
        query_raw: ":Agree".into(),
        gone: false,
        timeout_ms: 500,
    };

    let changed = click_ref(false, &CommandContext::default()).unwrap();
    let uncertain = click_ref(
        true,
        &CommandContext::default().with_wait_selector(Some(wait)),
    );
    let uncertain =
        serde_json::to_value(ErrorPayload::from_app_error(&uncertain.unwrap_err())).unwrap();

    assert_golden(
        json!({ "changed": changed, "uncertain": uncertain }),
        include_str!("../../../../../../tests/fixtures/contract/actions-click.json"),
        &["lease_hold_ms", "elapsed_ms"],
    );
}
