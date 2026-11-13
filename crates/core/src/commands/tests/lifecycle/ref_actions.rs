use crate::actions::action_request::ActionRequest;
use crate::actions::capability::{SCROLL, SCROLL_TO, SELECT, SET_FOCUS, SET_VALUE, TOGGLE};
use crate::adapter::{
    ActionOps, InputOps, NativeHandle, ObservationOps, PlatformAdapter, SystemOps,
};
use crate::commands::helpers::{RefArgs, test_support::save_one_ref_snapshot};
use crate::commands::scroll::{self, ScrollArgs};
use crate::commands::select::{self, SelectArgs};
use crate::commands::set_value::{self, SetValueArgs};
use crate::commands::tests::golden::assert_golden;
use crate::commands::{check, focus, scroll_to, toggle, uncheck};
use crate::context::CommandContext;
use crate::refs::{RefEntry, refs_test_support::HomeGuard};
use crate::{
    Action, ActionResult, AdapterError, AppError, Direction, ElementState, InteractionLease,
};
use serde_json::{Value, json};
use std::sync::Mutex;

struct Stub(Mutex<(bool, String)>);

impl Stub {
    fn state(&self) -> ElementState {
        let (checked, value) = &*self.0.lock().unwrap();
        let states: Vec<&str> = checked.then_some("checked").into_iter().collect();
        let value = (!value.is_empty()).then_some(value);
        let state = json!({"role": "checkbox", "states": states, "value": value, "enabled": true, "hidden": false, "offscreen": false});
        serde_json::from_value(state).unwrap()
    }
}

impl ObservationOps for Stub {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!(
        "checkbox",
        "Target",
        [SET_FOCUS, SCROLL_TO, SCROLL, SELECT, SET_VALUE, TOGGLE],
        adapter => adapter.state()
    );
    crate::adapter::saved_ref_scope!("checkbox", Some("Target"));
}

impl ActionOps for Stub {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        let (checked, value) = &mut *self.0.lock().unwrap();
        match &request.action {
            Action::Check => *checked = true,
            Action::Uncheck => *checked = false,
            Action::Toggle => *checked = !*checked,
            Action::SetValue(text) => value.clone_from(text),
            _ => {}
        }
        let step = json!({"label": "press", "outcome": "succeeded", "mechanism": "semantic_api", "verified": true});
        let result = json!({"action": request.action.name(), "steps": [step], "disposition": {"delivery": "delivered_verified", "retry": "unsafe"}});
        Ok(serde_json::from_value(result).unwrap())
    }
}

impl InputOps for Stub {}

impl SystemOps for Stub {
    crate::adapter::guarded_interaction_lease!();
}

type Command<'a> =
    &'a dyn Fn(RefArgs, &dyn PlatformAdapter, &CommandContext) -> Result<Value, AppError>;

fn run(capability: &str, checked: bool, command: Command) -> Value {
    let _home = HomeGuard::new();
    let snapshot_id = save_one_ref_snapshot("checkbox", capability);
    let args = RefArgs {
        ref_id: format!("@{snapshot_id}:e1"),
        timeout_ms: None,
    };
    let stub = Stub(Mutex::new((checked, String::new())));
    command(args, &stub, &CommandContext::default()).unwrap()
}

#[test]
fn focus_scroll_to_scroll_and_select_report_a_verified_delivery() {
    let scroll = |a: RefArgs, s: &dyn PlatformAdapter, c: &CommandContext| {
        let args = ScrollArgs {
            ref_id: a.ref_id,
            direction: Direction::Down,
            amount: 3,
            timeout_ms: None,
        };
        scroll::execute(args, s, c)
    };
    let select = |a: RefArgs, s: &dyn PlatformAdapter, c: &CommandContext| {
        let args = SelectArgs {
            ref_id: a.ref_id,
            value: "Choice".into(),
            timeout_ms: None,
        };
        select::execute(args, s, c)
    };
    let v = json!({
        "focus": run(SET_FOCUS, false, &focus::execute),
        "scroll_to": run(SCROLL_TO, false, &scroll_to::execute),
        "scroll": run(SCROLL, false, &scroll),
        "select": run(SELECT, false, &select),
    });
    const G: &str =
        include_str!("../../../../../../tests/fixtures/contract/lifecycle-delivery.json");
    assert_golden(v, G, &["lease_hold_ms"]);
}

#[test]
fn set_value_toggle_check_and_uncheck_report_the_state_they_read_back() {
    let set_value = |a: RefArgs, s: &dyn PlatformAdapter, c: &CommandContext| {
        let args = SetValueArgs {
            ref_id: a.ref_id,
            value: "hello".into(),
            timeout_ms: None,
        };
        set_value::execute(args, s, c)
    };
    let v = json!({
        "set_value": run(SET_VALUE, false, &set_value),
        "toggle": run(TOGGLE, false, &toggle::execute),
        "check": run(TOGGLE, false, &check::execute),
        "uncheck": run(TOGGLE, true, &uncheck::execute),
    });
    const G: &str = include_str!("../../../../../../tests/fixtures/contract/lifecycle-state.json");
    assert_golden(v, G, &["lease_hold_ms"]);
}
