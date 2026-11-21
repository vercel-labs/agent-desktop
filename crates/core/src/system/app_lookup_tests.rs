use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct InventoryAdapter {
    apps: Vec<AppInfo>,
    windows: Vec<crate::WindowInfo>,
}

impl ObservationOps for InventoryAdapter {
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _deadline: Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Ok(query.answer(self.apps.clone()))
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<crate::WindowInfo>, AdapterError> {
        Ok(self.windows.clone())
    }
}

impl ActionOps for InventoryAdapter {}
impl InputOps for InventoryAdapter {}
impl SystemOps for InventoryAdapter {}

fn app(instance: Option<&str>) -> AppInfo {
    AppInfo {
        aliases: Vec::new(),
        name: "Example".into(),
        pid: crate::ProcessId::new(42),
        bundle_id: Some("com.example.app".into()),
        process_instance: instance.map(str::to_string),
        presentation: None,
    }
}

fn app_with_pid(pid: u32, instance: Option<&str>) -> AppInfo {
    AppInfo {
        aliases: Vec::new(),
        pid: crate::ProcessId::new(pid),
        ..app(instance)
    }
}

struct ScopedInventorySpy {
    apps: Vec<AppInfo>,
    global_calls: AtomicUsize,
    scopes: Mutex<Vec<(String, Option<String>)>>,
}

impl ScopedInventorySpy {
    fn new(apps: Vec<AppInfo>) -> Self {
        Self {
            apps,
            global_calls: AtomicUsize::new(0),
            scopes: Mutex::new(Vec::new()),
        }
    }
}

impl ObservationOps for ScopedInventorySpy {
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _deadline: Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        let crate::AppInventoryQuery::Exact { name, bundle_id } = query else {
            self.global_calls.fetch_add(1, Ordering::Relaxed);
            return Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "unrelated global inventory failure",
            ));
        };
        self.scopes
            .lock()
            .unwrap()
            .push((name.to_string(), bundle_id.map(str::to_string)));
        Ok(crate::AppInventory {
            apps: self.apps.clone(),
            skipped: Vec::new(),
        })
    }
}

impl ActionOps for ScopedInventorySpy {}
impl InputOps for ScopedInventorySpy {}
impl SystemOps for ScopedInventorySpy {}

struct ScopedPermissionAdapter;

impl ObservationOps for ScopedPermissionAdapter {
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _deadline: Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Err(match query {
            crate::AppInventoryQuery::Exact { .. } => AdapterError::new(
                ErrorCode::PermDenied,
                "permission denied reading requested process identity",
            ),
            _ => AdapterError::new(
                ErrorCode::AppUnresponsive,
                "unrelated global inventory failure",
            ),
        })
    }
}

impl ActionOps for ScopedPermissionAdapter {}
impl InputOps for ScopedPermissionAdapter {}
impl SystemOps for ScopedPermissionAdapter {}

#[test]
fn named_resolution_asks_only_for_the_named_application() {
    let adapter = ScopedInventorySpy::new(vec![app(Some("generation-a"))]);

    let resolved = resolve_app(Some("example"), &adapter, Deadline::standard().unwrap()).unwrap();

    assert_eq!(resolved.pid, 42);
    assert_eq!(adapter.global_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        *adapter.scopes.lock().unwrap(),
        vec![("example".to_string(), None)]
    );
}

#[test]
fn mutation_revalidation_uses_name_and_bundle_scope() {
    let expected = app(Some("generation-a"));
    let mut differently_cased = expected.clone();
    differently_cased.name = "example".into();
    differently_cased.bundle_id = Some("COM.EXAMPLE.APP".into());
    let adapter = ScopedInventorySpy::new(vec![differently_cased]);

    let resolved =
        revalidate_app_for_mutation(&adapter, &expected, Deadline::standard().unwrap()).unwrap();

    assert_eq!(resolved.pid, 42);
    assert_eq!(adapter.global_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        *adapter.scopes.lock().unwrap(),
        vec![("Example".to_string(), Some("com.example.app".to_string()))]
    );
}

#[test]
fn scoped_target_permission_failure_is_preserved() {
    let error = resolve_app(
        Some("Example"),
        &ScopedPermissionAdapter,
        Deadline::standard().unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code(), "PERM_DENIED");
}

#[test]
fn an_exact_query_filters_name_and_bundle_case_insensitively() {
    let mut wrong_name = app_with_pid(43, Some("generation-b"));
    wrong_name.name = "Example Helper".into();
    let mut wrong_bundle = app_with_pid(44, Some("generation-c"));
    wrong_bundle.bundle_id = Some("com.example.other".into());
    let adapter = InventoryAdapter {
        apps: vec![app(Some("generation-a")), wrong_name, wrong_bundle],
        windows: Vec::new(),
    };

    let scoped = crate::AppInventoryQuery::Exact {
        name: "example",
        bundle_id: Some("COM.EXAMPLE.APP"),
    }
    .select(adapter.apps.clone());

    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].pid, 42);
}

#[test]
fn named_resolution_rejects_any_matching_candidate_without_generation() {
    let adapter = InventoryAdapter {
        apps: vec![app(Some("generation-a")), app(None)],
        windows: Vec::new(),
    };

    let error = resolve_app(Some("Example"), &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
}

#[test]
fn named_resolution_keeps_same_name_multiple_pids_ambiguous() {
    let adapter = InventoryAdapter {
        apps: vec![
            app_with_pid(42, Some("generation-a")),
            app_with_pid(43, Some("generation-b")),
        ],
        windows: Vec::new(),
    };

    let error = resolve_app(Some("Example"), &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
}

#[test]
fn omitted_app_with_no_focused_window_is_not_found() {
    let adapter = InventoryAdapter {
        apps: vec![app(Some("generation-a"))],
        windows: Vec::new(),
    };

    let error = resolve_app(None, &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "APP_NOT_FOUND");
}

#[test]
fn mutation_revalidation_rejects_pid_reuse() {
    let expected = app(Some("generation-a"));
    let adapter = InventoryAdapter {
        apps: vec![app(Some("generation-b"))],
        windows: Vec::new(),
    };

    let error = revalidate_app_for_mutation(&adapter, &expected, Deadline::standard().unwrap())
        .unwrap_err();

    assert_eq!(error.code(), "STALE_REF");
}

#[test]
fn mutation_revalidation_rejects_missing_generation() {
    let expected = app(Some("generation-a"));
    let adapter = InventoryAdapter {
        apps: vec![app(None)],
        windows: Vec::new(),
    };

    let error = revalidate_app_for_mutation(&adapter, &expected, Deadline::standard().unwrap())
        .unwrap_err();

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
}

#[test]
fn ambiguous_instance_suggestion_names_pids_instead_of_a_ref_retry() {
    let adapter = InventoryAdapter {
        apps: vec![
            app_with_pid(42, Some("generation-a")),
            app_with_pid(43, Some("generation-b")),
        ],
        windows: Vec::new(),
    };

    let error = resolve_app(Some("Example"), &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
    let suggestion = error
        .suggestion()
        .expect("ambiguous target has a suggestion");
    assert!(
        !suggestion.contains("ref"),
        "no command reached through resolve_app takes a ref to disambiguate: {suggestion}"
    );
    assert!(
        suggestion.contains("42") && suggestion.contains("43"),
        "suggestion must name the candidate pids the caller cannot select among: {suggestion}"
    );
}

#[test]
fn app_not_found_carries_a_suggestion_naming_the_accepted_forms() {
    let adapter = InventoryAdapter {
        apps: Vec::new(),
        windows: Vec::new(),
    };

    let error =
        resolve_app(Some("NoSuchApp"), &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "APP_NOT_FOUND");
    let suggestion = error
        .suggestion()
        .expect("an unknown application has a suggestion");
    assert!(suggestion.contains("list-apps"), "{suggestion}");
    assert!(
        !suggestion.contains(".exe") && !suggestion.contains("Windows"),
        "core words its suggestion for every platform: {suggestion}"
    );
    let AppError::Adapter(error) = error else {
        panic!("expected adapter error")
    };
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
}

#[test]
fn an_unknown_application_name_is_not_delivered_and_safe_to_retry() {
    let adapter = InventoryAdapter {
        apps: Vec::new(),
        windows: Vec::new(),
    };
    let error = match resolve_app(Some("Missing"), &adapter, Deadline::after(1_000).unwrap()) {
        Err(AppError::Adapter(error)) => error,
        other => panic!("expected an adapter error, got {other:?}"),
    };

    assert_eq!(error.code, ErrorCode::AppNotFound);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert_eq!(error.disposition.retry(), crate::RetryDisposition::Safe);
}

fn aliased(pid: u32, name: &str, alias: &str) -> AppInfo {
    AppInfo {
        aliases: vec![alias.into()],
        name: name.into(),
        ..app_with_pid(pid, Some("generation-a"))
    }
}

#[test]
fn a_name_two_applications_only_alias_is_ambiguous_not_a_silent_pick() {
    let adapter = InventoryAdapter {
        apps: vec![
            aliased(42, "gedit", "Text Editor"),
            aliased(43, "gnome-text-editor", "Text Editor"),
        ],
        windows: Vec::new(),
    };

    let error =
        resolve_app(Some("Text Editor"), &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
}

#[test]
fn the_application_carrying_the_name_wins_over_one_that_aliases_it() {
    let adapter = InventoryAdapter {
        apps: vec![aliased(42, "electron", "code"), aliased(43, "code", "")],
        windows: Vec::new(),
    };

    let app = resolve_app(Some("code"), &adapter, Deadline::standard().unwrap()).unwrap();

    assert_eq!(app.pid, crate::ProcessId::new(43));
}
