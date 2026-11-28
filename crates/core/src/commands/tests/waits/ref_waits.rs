use super::ref_stub::{RefStub, save_run_button};
use super::stub::{golden, payload};
use crate::commands::helpers::{RefArgs, execute_ref_action_with_context};
use crate::ref_action::{
    ref_action_wait::execute_with_auto_wait, ref_action_wait_context::RefActionWaitContext,
};
use crate::refs::refs_test_support::HomeGuard;
use crate::{adapter::live_element::incomplete_live_evidence, adapter::minimal_ref_entry, *};
use serde_json::{Value, json};

fn auto_wait_error(stub: &RefStub) -> Value {
    let mut entry = minimal_ref_entry("button", Some("Run"));
    entry.capabilities.available_actions = vec!["Click".into()];
    let context = RefActionWaitContext {
        adapter: stub,
        entry: &entry,
        ref_id: "@s1:e1",
        context: &CommandContext::default(),
    };
    let request = ActionRequest::headless(Action::Click).with_timeout_ms(Some(150));
    let error = execute_with_auto_wait(context, request).expect_err("the wait ends");
    payload(Err(AppError::Adapter(error)))
}

#[test]
fn actionability_live_read_and_deadline_timeouts_report_their_evidence() {
    let disabled = RefStub {
        disabled: true,
        ..Default::default()
    };
    let incomplete = RefStub {
        element_error: Some(incomplete_live_evidence()),
        ..Default::default()
    };
    let deadline = Deadline::after(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));

    let data = json!({
        "actionability": auto_wait_error(&disabled),
        "deadline": payload(Err(AppError::Adapter(deadline.timeout_error()))),
        "live_read_incomplete": auto_wait_error(&incomplete),
    });

    golden!(
        data,
        include_str!("../../../../../../tests/fixtures/contract/waits-ref-timeouts.json")
    );
}

#[test]
fn post_action_observation_reports_each_outcome() {
    let _home = HomeGuard::new();
    let uncertain_click = |stub: RefStub, gone: bool| {
        let id = save_run_button(Some("w-1"));
        let wait = WaitSelector {
            query_raw: ":Run".into(),
            gone,
            timeout_ms: 150,
        };
        let context = CommandContext::default().with_wait_selector(Some(wait));
        let args = RefArgs {
            ref_id: format!("@{id}:e1"),
            timeout_ms: None,
        };
        let request = ActionRequest::headless(Action::Click);
        payload(execute_ref_action_with_context(
            args, &stub, request, &context,
        ))
    };
    let failed = RefStub {
        tree_error: Some(AdapterError::permission_denied()),
        ..Default::default()
    };
    let exited = RefStub {
        closes_window: true,
        process_exited: true,
        ..Default::default()
    };
    let closed = RefStub {
        closes_window: true,
        ..Default::default()
    };

    let data = json!({
        "observation_failed": uncertain_click(failed, false),
        "process_exited": uncertain_click(exited, true),
        "window_gone": uncertain_click(closed, true),
    });

    golden!(
        data,
        include_str!("../../../../../../tests/fixtures/contract/waits-post-action.json")
    );
}
