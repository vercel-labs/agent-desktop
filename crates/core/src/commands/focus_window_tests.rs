use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, DeliverySemantics, WindowInfo};
use std::sync::Mutex;
use std::time::Duration;

struct FocusAdapter {
    windows: Vec<WindowInfo>,
    focused_windows: Mutex<Vec<WindowInfo>>,
    focused_window_calls: Mutex<u32>,
    focused_window_supported: bool,
    focus_sent: Mutex<bool>,
    focused_before: bool,
    focus_delivery: DeliverySemantics,
}

impl FocusAdapter {
    fn focus_holds(&self) -> bool {
        self.focused_before || *self.focus_sent.lock().unwrap()
    }
}

impl ObservationOps for FocusAdapter {
    fn list_windows(
        &self,
        filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        if filter.focused_only {
            if !self.focus_holds() {
                return Ok(Vec::new());
            }
            Ok(self.focused_windows.lock().unwrap().clone())
        } else {
            Ok(self.windows.clone())
        }
    }
}

impl ActionOps for FocusAdapter {}

impl InputOps for FocusAdapter {}

impl SystemOps for FocusAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn focus_window(
        &self,
        _win: &WindowInfo,
        _lease: &crate::InteractionLease,
    ) -> Result<DeliverySemantics, AdapterError> {
        *self.focus_sent.lock().unwrap() = true;
        Ok(self.focus_delivery)
    }

    fn focused_window(
        &self,
        _deadline: crate::Deadline,
    ) -> Result<Option<WindowInfo>, AdapterError> {
        *self.focused_window_calls.lock().unwrap() += 1;
        if !self.focused_window_supported {
            return Err(AdapterError::not_supported("focused_window"));
        }
        if !self.focus_holds() {
            return Ok(None);
        }
        let mut focused = self.focused_windows.lock().unwrap();
        if focused.len() > 1 {
            Ok(Some(focused.remove(0)))
        } else {
            Ok(focused.first().cloned())
        }
    }
}

fn window(id: &str, focused: bool) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "Main".into(),
        app: "TextEdit".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: focused,
            ..Default::default()
        },
    }
}

#[test]
fn reports_focused_window_after_os_confirms_focus() {
    let target = window("w1", false);
    let adapter = FocusAdapter {
        windows: vec![target.clone()],
        focused_windows: Mutex::new(vec![window("w1", true)]),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: true,
        focus_sent: Mutex::new(false),
        focused_before: false,
        focus_delivery: DeliverySemantics::delivered_unverified(),
    };

    let value = execute(
        FocusWindowArgs {
            window_id: Some(target.id),
            app: None,
            title: None,
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap();

    assert_eq!(
        value["disposition"],
        serde_json::json!({ "delivery": "delivered_verified", "retry": "unsafe" })
    );
    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 3);
}

#[test]
fn errors_when_focus_does_not_settle_on_requested_window() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let target = window("w1", false);
    let adapter = FocusAdapter {
        windows: vec![target.clone()],
        focused_windows: Mutex::new(Vec::new()),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: true,
        focus_sent: Mutex::new(false),
        focused_before: false,
        focus_delivery: DeliverySemantics::delivered_unverified(),
    };

    let err = execute(
        FocusWindowArgs {
            window_id: Some(target.id),
            app: None,
            title: None,
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ACTION_FAILED");
    let crate::AppError::Adapter(error) = err else {
        panic!("a focus settle failure is an adapter error");
    };
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_failed_confirmation_after_a_dispatch_that_sent_nothing_stays_not_delivered() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let target = window("w1", false);
    let adapter = FocusAdapter {
        windows: vec![target.clone()],
        focused_windows: Mutex::new(Vec::new()),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: true,
        focus_sent: Mutex::new(false),
        focused_before: false,
        focus_delivery: DeliverySemantics::not_delivered(),
    };

    let err = execute(
        FocusWindowArgs {
            window_id: Some(target.id),
            app: None,
            title: None,
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ACTION_FAILED");
    let crate::AppError::Adapter(error) = err else {
        panic!("a focus settle failure is an adapter error");
    };
    assert_eq!(
        error.disposition,
        DeliverySemantics::not_delivered(),
        "the adapter sent nothing, so the confirmation failure cannot be an unknown delivery"
    );
    assert_eq!(error.disposition.retry(), crate::RetryDisposition::Safe);
}

#[test]
fn falls_back_to_focused_window_list_when_direct_observation_is_unsupported() {
    let target = window("w1", false);
    let adapter = FocusAdapter {
        windows: vec![target.clone()],
        focused_windows: Mutex::new(vec![window("w1", true)]),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: false,
        focus_sent: Mutex::new(false),
        focused_before: false,
        focus_delivery: DeliverySemantics::delivered_unverified(),
    };

    let value = execute(
        FocusWindowArgs {
            window_id: Some(target.id),
            app: None,
            title: None,
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap();

    assert_eq!(value["focused"]["id"], "w1");
    assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 3);
}

#[test]
fn focus_confirmation_resets_after_transient_wrong_window() {
    let target = window("w1", false);
    let adapter = FocusAdapter {
        windows: vec![target.clone()],
        focused_windows: Mutex::new(vec![
            window("w1", true),
            window("w2", true),
            window("w1", true),
            window("w1", true),
        ]),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: true,
        focus_sent: Mutex::new(false),
        focused_before: true,
        focus_delivery: DeliverySemantics::delivered_unverified(),
    };

    let value = crate::system::window_focus::wait_for_focused_window_with_poll_interval(
        &adapter,
        &target.id,
        None,
        Duration::ZERO,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap();

    assert_eq!(value.id, "w1");
    assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 4);
}

#[test]
fn app_filter_results_are_not_discarded_by_display_name() {
    let mut target = window("w1", false);
    target.app = "WhatsApp".into();
    let mut focused = target.clone();
    focused.state.is_focused = true;
    let adapter = FocusAdapter {
        windows: vec![target],
        focused_windows: Mutex::new(vec![focused]),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: true,
        focus_sent: Mutex::new(false),
        focused_before: false,
        focus_delivery: DeliverySemantics::delivered_unverified(),
    };

    let value = execute(
        FocusWindowArgs {
            window_id: None,
            app: Some("net.whatsapp.WhatsApp".into()),
            title: None,
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap();

    assert_eq!(value["focused"]["id"], "w1");
}

#[test]
fn headless_focus_window_is_refused_before_any_window_is_read() {
    let target = window("w1", false);
    let adapter = FocusAdapter {
        windows: vec![target.clone()],
        focused_windows: Mutex::new(vec![window("w1", true)]),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: true,
        focus_sent: Mutex::new(false),
        focused_before: false,
        focus_delivery: DeliverySemantics::delivered_unverified(),
    };
    let err = execute(
        FocusWindowArgs {
            window_id: Some(target.id),
            app: None,
            title: None,
        },
        &adapter,
        crate::InteractionPolicy::headless(),
    )
    .unwrap_err();
    let crate::AppError::Adapter(err) = err else {
        panic!("a policy refusal is an adapter error");
    };
    assert_eq!(err.code, crate::ErrorCode::PolicyDenied);
    assert_eq!(err.details.unwrap()["effect"], "focus_steal");
    assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 0);
}

#[test]
fn a_window_that_is_already_focused_is_not_delivered_and_nothing_is_sent() {
    let target = window("w1", true);
    let adapter = FocusAdapter {
        windows: vec![target.clone()],
        focused_windows: Mutex::new(vec![target.clone()]),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: true,
        focus_sent: Mutex::new(false),
        focused_before: true,
        focus_delivery: DeliverySemantics::delivered_unverified(),
    };

    let value = execute(
        FocusWindowArgs {
            window_id: Some(target.id),
            app: None,
            title: None,
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap();

    assert_eq!(
        value["disposition"],
        serde_json::json!({ "delivery": "not_delivered", "retry": "safe" })
    );
    assert!(!*adapter.focus_sent.lock().unwrap());
}

#[test]
fn a_dispatch_that_sent_nothing_is_not_delivered_even_once_the_window_is_focused() {
    let target = window("w1", false);
    let adapter = FocusAdapter {
        windows: vec![target.clone()],
        focused_windows: Mutex::new(vec![window("w1", true)]),
        focused_window_calls: Mutex::new(0),
        focused_window_supported: true,
        focus_sent: Mutex::new(false),
        focused_before: false,
        focus_delivery: DeliverySemantics::not_delivered(),
    };

    let value = execute(
        FocusWindowArgs {
            window_id: Some(target.id),
            app: None,
            title: None,
        },
        &adapter,
        crate::InteractionPolicy::headed(),
    )
    .unwrap();

    assert_eq!(
        value["disposition"],
        serde_json::json!({ "delivery": "not_delivered", "retry": "safe" })
    );
    assert_eq!(value["focused"]["id"], "w1");
}
