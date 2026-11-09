use crate::commands::wait::{WaitArgs, WaitModeArgs, WaitPredicateArgs, execute};
use crate::{adapter::*, *};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::Mutex;

macro_rules! golden {
    ($value:expr, $golden:expr) => {
        $crate::commands::tests::golden::assert_golden($value, $golden, &["elapsed_ms"])
    };
}
pub(super) use golden;

macro_rules! mode {
    ($field:ident, $value:expr) => {
        $crate::commands::wait::WaitModeArgs {
            $field: Some($value.into()),
            ..Default::default()
        }
    };
}
pub(super) use mode;

pub(super) fn build<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("the value deserializes")
}

pub(super) fn script<T>(items: Vec<T>) -> Mutex<VecDeque<T>> {
    Mutex::new(items.into())
}

fn next<T: Clone>(queue: &Mutex<VecDeque<T>>) -> Option<T> {
    let mut queue = queue.lock().unwrap();
    match queue.len() > 1 {
        true => queue.pop_front(),
        false => queue.front().cloned(),
    }
}

pub(super) fn window(id: &str, title: &str) -> WindowInfo {
    build(
        json!({"id": id, "title": title, "app_name": "TextEdit", "pid": 42,
        "process_instance": "test-instance", "is_focused": true}),
    )
}

pub(super) fn app() -> AppInfo {
    build(json!({"name": "TextEdit", "pid": 42, "process_instance": "test-instance"}))
}

pub(super) fn signals(windows: Vec<WindowInfo>, apps: Vec<AppInfo>) -> SignalBaseline {
    let completeness = SignalCompleteness::complete();
    SignalBaseline {
        windows,
        apps,
        surfaces: Vec::new(),
        completeness,
    }
}

#[derive(Default)]
pub(super) struct Stub {
    pub(super) windows: Mutex<VecDeque<Result<Vec<WindowInfo>, AdapterError>>>,
    pub(super) tree: Option<AccessibilityNode>,
    pub(super) notifications: Mutex<VecDeque<Result<Vec<NotificationInfo>, AdapterError>>>,
    pub(super) menu_times_out: bool,
    pub(super) signals: SignalBaseline,
}

impl ObservationOps for Stub {
    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        let default = minimal_accessibility_node("window", "Doc");
        observed_tree(&root, self.tree.clone().unwrap_or(default))
    }

    fn list_windows(&self, _: &WindowFilter, _: Deadline) -> Result<Vec<WindowInfo>, AdapterError> {
        next(&self.windows).unwrap_or_else(|| Ok(vec![window("w-1", "Doc")]))
    }

    fn query_app_inventory(
        &self,
        query: AppInventoryQuery<'_>,
        _: Deadline,
    ) -> Result<AppInventory, AdapterError> {
        Ok(query.answer(vec![app()]))
    }
}

impl ActionOps for Stub {}
impl InputOps for Stub {}

impl SystemOps for Stub {
    fn wait_for_menu(
        &self,
        _: ProcessIdentity,
        _: bool,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        match self.menu_times_out {
            true => Err(deadline.timeout_error()),
            false => Ok(()),
        }
    }

    fn list_notifications(
        &self,
        _: &NotificationFilter,
        _: InteractionPolicy,
        _: Deadline,
        _: Option<&InteractionLease>,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        next(&self.notifications)
            .unwrap_or_else(|| Err(AdapterError::timeout("notification center unavailable")))
    }

    fn capture_signal_baseline(
        &self,
        _: &SignalFilter,
        _: Deadline,
    ) -> Result<SignalBaseline, AdapterError> {
        Ok(self.signals.clone())
    }
}

pub(super) fn with_buttons(names: &[&str]) -> Stub {
    let mut tree = minimal_accessibility_node("window", "Doc");
    tree.children = names
        .iter()
        .map(|name| minimal_accessibility_node("button", name))
        .collect();
    Stub {
        tree: Some(tree),
        ..Default::default()
    }
}

pub(super) fn args(mode: WaitModeArgs, app: Option<&str>, timeout_ms: u64) -> WaitArgs {
    let predicate = WaitPredicateArgs {
        predicate: None,
        value: None,
        action: None,
        count: None,
    };
    let app = app.map(str::to_string);
    WaitArgs {
        mode,
        predicate,
        timeout_ms,
        app,
    }
}

pub(super) fn run(
    adapter: &dyn PlatformAdapter,
    mode: WaitModeArgs,
    app: Option<&str>,
    timeout_ms: u64,
) -> Result<Value, AppError> {
    let wait = args(mode, app, timeout_ms);
    execute(wait, adapter, &CommandContext::default())
}

pub(super) fn selector(
    adapter: &dyn PlatformAdapter,
    query: &str,
    gone: bool,
    timeout_ms: u64,
) -> Result<Value, AppError> {
    let input = crate::commands::wait_selector::WaitSelectorInput {
        query_raw: query.into(),
        gone,
        app: Some("TextEdit".into()),
        window_id: None,
        opts: TreeOptions::default(),
        timeout_ms,
    };
    crate::commands::wait_selector::execute(input, adapter, &CommandContext::default())
}

pub(super) fn payload(result: Result<Value, AppError>) -> Value {
    let error = result.expect_err("the wait must end in an error");
    serde_json::to_value(ErrorPayload::from_app_error(&error)).expect("payload serializes")
}

pub(super) fn stable(value: Value, extra_ids: &[String]) -> Value {
    let mut text = value.to_string();
    let found = text
        .split("\"snapshot_id\":\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for id in found.iter().chain(extra_ids) {
        text = text.replace(id.as_str(), "SNAPSHOT");
    }
    serde_json::from_str(&text).expect("normalized value parses")
}
