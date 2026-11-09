use super::dispatch;
use super::test_support::HomeGuard;
use super::wait_contract_stub::{self, WaitStub};
use crate::cli::Cli;
use agent_desktop_core::context::{CommandContext, WaitSelector};
use agent_desktop_core::{AdapterError, AppError, ErrorCode, ErrorPayload, PermissionReport};
use clap::Parser;
use serde_json::{Value, json};

const TIMEOUTS_GOLDEN: &str = include_str!("../../tests/fixtures/contract/waits-timeouts.json");
const ELEMENT_GOLDEN: &str = include_str!("../../tests/fixtures/contract/waits-element.json");

fn run(
    arguments: &[&str],
    adapter: &WaitStub,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let mut argv = vec!["agent-desktop"];
    argv.extend_from_slice(arguments);
    let command = Cli::try_parse_from(argv)
        .expect("command parses")
        .command
        .expect("subcommand is present");
    dispatch(command, adapter, &PermissionReport::default(), context)
}

fn payload(result: Result<Value, AppError>) -> Value {
    let error = result.expect_err("the wait must end in an error");
    serde_json::to_value(ErrorPayload::from_app_error(&error)).expect("payload serializes")
}

fn wait_selector_context(query: &str, timeout_ms: u64) -> CommandContext {
    CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: query.into(),
        gone: false,
        timeout_ms,
    }))
}

fn stable(value: Value, ids: &[String]) -> Value {
    let mut text = value.to_string();
    let found = text
        .split("\"snapshot_id\":\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for id in found.iter().chain(ids.iter()) {
        text = text.replace(id.as_str(), "SNAPSHOT");
    }
    serde_json::from_str(&text).expect("normalized value parses")
}

fn assert_golden(mut actual: Value, golden: &str, varying: &[&str]) {
    for key in varying {
        redact(&mut actual, key);
    }
    let expected: Value = serde_json::from_str(golden).expect("the golden file is valid JSON");
    assert_eq!(
        actual,
        expected,
        "output differs from the golden file. Actual:\n{}",
        serde_json::to_string_pretty(&actual).unwrap_or_default()
    );
}

fn redact(value: &mut Value, key: &str) {
    match value {
        Value::Object(map) => {
            for (name, inner) in map.iter_mut() {
                if name == key {
                    *inner = match inner {
                        Value::Number(_) => Value::from(0),
                        Value::Bool(_) => Value::Bool(false),
                        Value::Array(_) => Value::Array(Vec::new()),
                        Value::Object(_) => Value::Object(Default::default()),
                        Value::Null => Value::Null,
                        Value::String(_) => Value::String("<varies>".into()),
                    };
                } else {
                    redact(inner, key);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| redact(item, key)),
        _ => {}
    }
}

#[test]
fn wait_timeouts_report_their_wire_evidence() {
    let _home = HomeGuard::new();
    let context =
        CommandContext::default().with_event_baseline(Some(Err(wait_contract_stub::seed_error())));
    let menu = WaitStub {
        menu_times_out: true,
        ..Default::default()
    };
    let selector_context = wait_selector_context("button:Save", 100);

    let data = json!({
        "event": payload(run(&["wait", "--event", "window-opened", "--timeout", "100"], &WaitStub::default(), &context)),
        "menu": payload(run(&["wait", "--menu", "--app", "TextEdit", "--timeout", "2000"], &menu, &CommandContext::default())),
        "notification": payload(run(&["wait", "--notification", "--timeout", "100"], &WaitStub::default(), &CommandContext::default())),
        "selector_last_error": payload(run(&["snapshot", "--app", "TextEdit"], &WaitStub::unreachable(), &selector_context)),
        "selector_snapshot": payload(run(&["snapshot", "--app", "TextEdit"], &WaitStub::with_buttons(&["Cancel"]), &selector_context)),
        "text": payload(run(&["wait", "--text", "Quarterly", "--app", "TextEdit", "--timeout", "100"], &WaitStub::rebuilding(), &CommandContext::default())),
        "window": payload(run(&["wait", "--window", "Quarterly", "--app", "TextEdit", "--timeout", "250"], &WaitStub::rebuilding(), &CommandContext::default())),
    });

    assert_golden(stable(data, &[]), TIMEOUTS_GOLDEN, &["elapsed_ms"]);
}

#[test]
fn element_wait_reports_the_actionability_and_the_last_observed_error() {
    let _home = HomeGuard::new();
    let context = CommandContext::default();
    let (snapshot_id, ref_id) = wait_contract_stub::save_run_button(&context);
    let arguments = [
        "wait",
        "--element",
        ref_id.as_str(),
        "--predicate",
        "actionable",
        "--timeout",
        "150",
    ];
    let missing = WaitStub {
        element_error: Some(AdapterError::new(
            ErrorCode::ElementNotFound,
            "The element is not on screen yet",
        )),
        ..Default::default()
    };

    let found = run(&arguments, &WaitStub::default(), &context).expect("the element wait finds");
    let timeout = payload(run(&arguments, &missing, &context));
    let data = json!({ "found": found, "timeout": timeout });

    assert_golden(
        stable(data, std::slice::from_ref(&snapshot_id)),
        ELEMENT_GOLDEN,
        &["elapsed_ms"],
    );
}
