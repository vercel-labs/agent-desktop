use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::system::launch_options::LaunchOptions;
use crate::{
    AdapterError, InteractionLease, LaunchFacts, LaunchGuidance, ProcessId, WindowInfo, WindowState,
};
use serde_json::json;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

struct LaunchAdapter {
    lease_timeout_ms: AtomicU64,
    list_apps_calls: AtomicUsize,
    facts: LaunchFacts,
}

const FRESH: LaunchFacts = LaunchFacts {
    attached: false,
    activation_dispatched: false,
    postcondition_observed: true,
};

fn adapter_with(facts: LaunchFacts) -> LaunchAdapter {
    LaunchAdapter {
        lease_timeout_ms: AtomicU64::new(0),
        list_apps_calls: AtomicUsize::new(0),
        facts,
    }
}

fn fixture_window() -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Fixture".into(),
        app: "Fixture".into(),
        pid: ProcessId::new(42),
        process_instance: Some("42:1".into()),
        bounds: None,
        state: WindowState::default(),
    }
}

fn launch(adapter: &LaunchAdapter, activate: bool) -> serde_json::Value {
    execute(
        LaunchArgs {
            app: "Fixture".into(),
            options: LaunchOptions {
                activate,
                ..LaunchOptions::default()
            },
        },
        adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap()
}

impl ObservationOps for LaunchAdapter {
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _deadline: crate::Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Ok(query.answer({
            self.list_apps_calls.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }?))
    }
}
impl ActionOps for LaunchAdapter {}
impl InputOps for LaunchAdapter {}

impl SystemOps for LaunchAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        self.lease_timeout_ms
            .store(deadline.timeout_ms(), Ordering::SeqCst);
        InteractionLease::guarded(deadline, ())
    }

    fn launch_app(
        &self,
        _id: &str,
        _options: &LaunchOptions,
        _lease: &InteractionLease,
    ) -> Result<crate::system::launch_result::LaunchResult, AdapterError> {
        Ok(crate::system::launch_result::LaunchResult {
            app: "Fixture".into(),
            pid: ProcessId::new(42),
            process_instance: Some("42:1".into()),
            window: Some(fixture_window()),
            guidance: LaunchGuidance::default(),
            disposition: self.facts.disposition(),
        })
    }
}

#[test]
fn launch_lease_uses_the_requested_timeout() {
    let adapter = adapter_with(FRESH);
    let options = LaunchOptions {
        timeout_ms: 30_000,
        ..Default::default()
    };

    execute(
        LaunchArgs {
            app: "Fixture".into(),
            options,
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap();

    assert_eq!(adapter.lease_timeout_ms.load(Ordering::SeqCst), 30_000);
}

#[test]
fn zero_launch_timeout_keeps_a_bounded_lease_for_the_single_attempt() {
    let adapter = adapter_with(FRESH);

    execute(
        LaunchArgs {
            app: "Fixture".into(),
            options: LaunchOptions {
                timeout_ms: 0,
                ..Default::default()
            },
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap();

    assert_eq!(
        adapter.lease_timeout_ms.load(Ordering::SeqCst),
        crate::Deadline::standard().unwrap().timeout_ms()
    );
}

/// A launch that never asks for `--cdp` must cost nothing extra: no
/// running-app lookup, no argument mutation. Regression guard against the
/// cdp precheck accidentally running unconditionally.
#[test]
fn plain_launch_without_cdp_never_calls_list_apps() {
    let adapter = adapter_with(FRESH);

    execute(
        LaunchArgs {
            app: "Fixture".into(),
            options: LaunchOptions::default(),
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap();

    assert_eq!(adapter.list_apps_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn headless_launch_with_activate_is_refused_before_anything_is_launched() {
    let adapter = adapter_with(FRESH);
    let options = LaunchOptions {
        activate: true,
        ..LaunchOptions::default()
    };
    let error = execute(
        LaunchArgs {
            app: "Fixture".into(),
            options,
        },
        &adapter,
        crate::InteractionPolicy::headless(),
    )
    .unwrap_err();
    let crate::AppError::Adapter(error) = error else {
        panic!("a policy refusal is an adapter error");
    };
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(error.details.unwrap()["effect"], "focus_steal");
    assert_eq!(adapter.lease_timeout_ms.load(Ordering::SeqCst), 0);
}

fn expected_data(delivery: &str, retry: &str) -> serde_json::Value {
    json!({
        "app": "Fixture",
        "pid": 42,
        "process_instance": "42:1",
        "window": serde_json::to_value(fixture_window()).unwrap(),
        "disposition": { "delivery": delivery, "retry": retry },
    })
}

#[test]
fn attaching_without_activation_reports_not_delivered() {
    let adapter = adapter_with(LaunchFacts {
        attached: true,
        activation_dispatched: false,
        postcondition_observed: false,
    });

    assert_eq!(
        launch(&adapter, false),
        expected_data("not_delivered", "safe")
    );
}

#[test]
fn a_fresh_launch_whose_instance_was_read_back_reports_delivered_verified() {
    let adapter = adapter_with(FRESH);

    assert_eq!(
        launch(&adapter, false),
        expected_data("delivered_verified", "unsafe")
    );
}

#[test]
fn attaching_with_activation_reports_delivered_unverified() {
    let adapter = adapter_with(LaunchFacts {
        attached: true,
        activation_dispatched: true,
        postcondition_observed: false,
    });

    assert_eq!(
        launch(&adapter, true),
        expected_data("delivered_unverified", "unsafe")
    );
}

#[test]
fn attaching_with_activation_and_a_confirmed_frontmost_reports_delivered_verified() {
    let adapter = adapter_with(LaunchFacts {
        attached: true,
        activation_dispatched: true,
        postcondition_observed: true,
    });

    assert_eq!(
        launch(&adapter, true),
        expected_data("delivered_verified", "unsafe")
    );
}
