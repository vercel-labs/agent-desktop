use super::*;
use agent_desktop_core::DeliveryDisposition;

#[test]
fn key_dispatch_never_calls_activation_apis() {
    let src = include_str!("key_dispatch.rs");
    assert!(
        !src.contains("focus_window"),
        "verify-only path must not call focus_window"
    );
    assert!(
        !src.contains("window_activate"),
        "verify-only path must not call window_activate"
    );
    assert!(
        !src.contains("SetForegroundWindow"),
        "verify-only path must not raise via SetForegroundWindow"
    );
}

#[test]
fn all_three_interaction_policies_are_constructible_and_distinct() {
    let headless = InteractionPolicy::headless();
    let focus_fallback = InteractionPolicy::focus_fallback();
    let headed = InteractionPolicy::headed();
    assert!(!headless.allow_focus_steal);
    assert!(!headless.allow_cursor_move);
    assert!(focus_fallback.allow_focus_steal);
    assert!(!focus_fallback.allow_cursor_move);
    assert!(!focus_fallback.is_headed());
    assert!(headed.allow_focus_steal);
    assert!(headed.allow_cursor_move);
    assert!(headed.is_headed());
}

#[cfg(target_os = "windows")]
mod delivery_gates {
    use super::*;
    use crate::input::keyboard_send_fake_sink as sink;
    use crate::system::test_support::live_identity::live_process_identity;
    use crate::system::test_support::test_time::deadline;

    fn combo_a() -> KeyCombo {
        KeyCombo {
            key: "a".into(),
            modifiers: vec![],
        }
    }

    fn own_identity() -> ProcessIdentity {
        live_process_identity(ProcessId::from(std::process::id()))
    }

    fn assert_not_delivered_before_any_send(error: &AdapterError) {
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::NotDelivered
        );
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("physical_delivery_started")),
            Some(&serde_json::Value::Bool(false))
        );
        assert!(
            sink::recorded().is_empty(),
            "no key may be synthesized when delivery is refused"
        );
    }

    #[test]
    fn a_headless_background_target_is_refused_before_any_key_is_sent() {
        sink::reset();
        let error = press_for_app_impl(
            own_identity(),
            &combo_a(),
            InteractionPolicy::headless(),
            deadline(10_000),
        )
        .expect_err("a process that does not own the foreground cannot be typed into");
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert!(
            error.message.contains("not foreground"),
            "headless names the background refusal: {}",
            error.message
        );
        assert_not_delivered_before_any_send(&error);
    }

    #[test]
    fn a_headed_target_that_lost_the_foreground_reports_the_focus_failure_without_a_key() {
        sink::reset();
        let error = press_for_app_impl(
            own_identity(),
            &combo_a(),
            InteractionPolicy::headed(),
            deadline(10_000),
        )
        .expect_err("a headed press still needs the target in the foreground");
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert!(
            error.message.contains("lost focus"),
            "headed names the lost focus: {}",
            error.message
        );
        assert!(error.suggestion.is_some());
        assert_not_delivered_before_any_send(&error);
    }

    #[test]
    fn a_process_instance_that_no_longer_runs_is_refused_before_any_key_is_sent() {
        sink::reset();
        let stale = ProcessIdentity::new(
            ProcessId::from(std::process::id()),
            "not-the-live-generation",
        );
        let error = press_for_app_impl(
            stale,
            &combo_a(),
            InteractionPolicy::headed(),
            deadline(10_000),
        )
        .expect_err("a recycled pid must not receive keys");
        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("physical_delivery_started")),
            Some(&serde_json::Value::Bool(false))
        );
        assert!(sink::recorded().is_empty());
    }
}
