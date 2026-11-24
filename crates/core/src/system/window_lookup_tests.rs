use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::sync::Mutex;

struct WindowInventory(Vec<WindowInfo>);

impl ObservationOps for WindowInventory {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, crate::AdapterError> {
        Ok(self.0.clone())
    }
}

impl ActionOps for WindowInventory {}
impl InputOps for WindowInventory {}
impl SystemOps for WindowInventory {}

struct ScopedWindowInventory {
    unfiltered: Vec<WindowInfo>,
    scoped: Vec<WindowInfo>,
    seen_app_filters: Mutex<Vec<Option<String>>>,
}

impl ScopedWindowInventory {
    fn new(unfiltered: Vec<WindowInfo>, scoped: Vec<WindowInfo>) -> Self {
        Self {
            unfiltered,
            scoped,
            seen_app_filters: Mutex::new(Vec::new()),
        }
    }
}

impl ObservationOps for ScopedWindowInventory {
    fn list_windows(
        &self,
        filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, crate::AdapterError> {
        self.seen_app_filters
            .lock()
            .expect("filters lock")
            .push(filter.app.clone());
        if filter.app.is_none() {
            return Ok(self.unfiltered.clone());
        }
        Ok(self.scoped.clone())
    }
}

impl ActionOps for ScopedWindowInventory {}
impl InputOps for ScopedWindowInventory {}
impl SystemOps for ScopedWindowInventory {}

fn window(instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Example".into(),
        app: "Example".into(),
        pid: crate::ProcessId::new(42),
        process_instance: instance.map(str::to_string),
        bounds: None,
        state: crate::WindowState {
            is_focused: true,
            ..Default::default()
        },
    }
}

#[test]
fn matching_pid_with_unknown_generation_fails_closed() {
    let adapter = WindowInventory(vec![window(Some("generation-a")), window(None)]);

    let error = find_window_for_process(
        ProcessIdentity::new(42, "generation-a"),
        Some("Example"),
        &adapter,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
}

#[test]
fn scoped_app_lookup_uses_app_filter_and_reports_window_not_found_when_only_phantom_windows_exist()
{
    let mut phantom_one = window(Some("generation-a"));
    phantom_one.id = "w-101".into();
    phantom_one.app = "TextEdit".into();
    phantom_one.state.is_focused = false;
    phantom_one.state.visible = Some(false);
    phantom_one.state.accessible = false;
    let mut phantom_two = phantom_one.clone();
    phantom_two.id = "w-102".into();

    let adapter = ScopedWindowInventory::new(vec![phantom_one, phantom_two], Vec::new());

    let error = find_window_for_process(
        ProcessIdentity::new(42, "generation-a"),
        Some("TextEdit"),
        &adapter,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap_err();

    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(
        error.message,
        "Application 'TextEdit' is running but has no matching window"
    );
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert_eq!(error.disposition.retry(), crate::RetryDisposition::Safe);
    assert_eq!(
        *adapter.seen_app_filters.lock().expect("filters lock"),
        vec![Some("TextEdit".to_string())]
    );
}

#[test]
fn scoped_app_lookup_selects_real_window_when_unfiltered_inventory_has_extra_phantoms() {
    let mut phantom_one = window(Some("generation-a"));
    phantom_one.id = "w-101".into();
    phantom_one.app = "TextEdit".into();
    phantom_one.state.is_focused = false;
    phantom_one.state.visible = Some(false);
    phantom_one.state.accessible = false;
    let mut phantom_two = phantom_one.clone();
    phantom_two.id = "w-102".into();

    let mut real = window(Some("generation-a"));
    real.id = "w-200".into();
    real.app = "TextEdit".into();
    real.state.is_focused = false;
    real.state.visible = Some(true);

    let adapter = ScopedWindowInventory::new(vec![phantom_one, phantom_two], vec![real]);

    let selected = find_window_for_process(
        ProcessIdentity::new(42, "generation-a"),
        Some("TextEdit"),
        &adapter,
        crate::Deadline::standard().unwrap(),
    )
    .expect("scoped lookup should return the single real window");

    assert_eq!(selected.id, "w-200");
    assert_eq!(
        *adapter.seen_app_filters.lock().expect("filters lock"),
        vec![Some("TextEdit".to_string())]
    );
}

#[test]
fn shared_window_selector_emits_one_stable_ambiguity_shape() {
    let mut first = window(Some("generation-a"));
    first.state.is_focused = false;
    let mut second = first.clone();
    second.id = "w-2".into();
    second.state.is_focused = false;
    let error = select_window(
        vec![first, second],
        crate::AdapterError::new(ErrorCode::WindowNotFound, "none"),
        "multiple",
    )
    .unwrap_err();

    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["candidate_count"], 2);
    assert_eq!(details["candidate_summaries_truncated"], false);
    assert_eq!(details["candidates"][0]["id"], "w-1");
    assert_eq!(details["candidates"][0]["is_focused"], false);
    assert!(details["candidates"][0].get("focused").is_none());
    assert!(details["candidates"][0].get("pid").is_none());
    assert!(details["candidates"][0].get("process_instance").is_none());
    assert!(error.suggestion.as_deref().unwrap().contains("--window-id"));
}

#[test]
fn shared_window_selector_prefers_the_only_visible_window() {
    let mut hidden = window(Some("generation-a"));
    hidden.state.is_focused = false;
    hidden.state.visible = Some(false);
    let mut visible = hidden.clone();
    visible.id = "w-2".into();
    visible.state.visible = Some(true);

    let selected = select_window(
        vec![hidden, visible],
        crate::AdapterError::new(ErrorCode::WindowNotFound, "none"),
        "multiple",
    )
    .unwrap();

    assert_eq!(selected.id, "w-2");
}

#[test]
fn shared_window_selector_prefers_the_visible_focused_window_over_an_accessible_hidden_one() {
    let mut visible_focused = window(Some("generation-a"));
    visible_focused.id = "w-visible-focused".into();
    visible_focused.state.visible = Some(true);
    visible_focused.state.is_focused = true;
    visible_focused.state.accessible = false;

    let mut hidden_accessible = window(Some("generation-a"));
    hidden_accessible.id = "w-hidden-accessible".into();
    hidden_accessible.state.visible = Some(false);
    hidden_accessible.state.is_focused = false;
    hidden_accessible.state.accessible = true;

    let mut irrelevant = window(Some("generation-a"));
    irrelevant.id = "w-irrelevant".into();
    irrelevant.state.visible = Some(false);
    irrelevant.state.is_focused = false;
    irrelevant.state.accessible = false;

    let selected = select_window(
        vec![hidden_accessible, irrelevant, visible_focused],
        crate::AdapterError::new(ErrorCode::WindowNotFound, "none"),
        "multiple",
    )
    .unwrap();

    assert_eq!(selected.id, "w-visible-focused");
}

#[test]
fn shared_window_selector_prefers_an_accessible_window() {
    let accessible = window(Some("generation-a"));
    let mut inaccessible = accessible.clone();
    inaccessible.id = "w-2".into();
    inaccessible.state.accessible = false;

    let selected = select_window(
        vec![inaccessible, accessible],
        crate::AdapterError::new(ErrorCode::WindowNotFound, "none"),
        "multiple",
    )
    .unwrap();

    assert_eq!(selected.id, "w-1");
}

#[test]
fn an_empty_window_selection_is_not_delivered_and_safe_to_retry() {
    let error = match select_window(
        Vec::new(),
        crate::AdapterError::new(ErrorCode::WindowNotFound, "none"),
        "many",
    ) {
        Err(AppError::Adapter(error)) => error,
        other => panic!("expected an adapter error, got {other:?}"),
    };

    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert_eq!(error.disposition.retry(), crate::RetryDisposition::Safe);
}
