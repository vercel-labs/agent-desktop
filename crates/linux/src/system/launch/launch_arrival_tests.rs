use agent_desktop_core::system::launch_options::LaunchOptions;
use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use agent_desktop_core::{ProcessId, RetryDisposition, WindowInfo};

use super::Arrival;
use crate::system::launch::conclude;
use crate::system::launch::launch_conclude::FocusAnswer;
use crate::system::process::running_app::Running;

fn owner() -> Running {
    Running {
        executable: None,
        pid: 4_000_000,
        instance: "boot:9".to_owned(),
        name: "tool".to_owned(),
    }
}

fn window() -> WindowInfo {
    WindowInfo {
        id: "w-1".to_owned(),
        title: "Tool".to_owned(),
        app: "tool".to_owned(),
        pid: ProcessId::new(4_000_000),
        process_instance: Some("boot:9".to_owned()),
        bounds: None,
        state: Default::default(),
    }
}

fn activating() -> LaunchOptions {
    LaunchOptions {
        activate: true,
        ..LaunchOptions::default()
    }
}

fn focus_answer(window: Option<WindowInfo>, dispatched: bool) -> FocusAnswer {
    FocusAnswer { window, dispatched }
}

fn disposition(arrival: Arrival) -> DeliverySemantics {
    arrival.facts().disposition()
}

#[test]
fn a_started_launch_is_classified_by_who_owns_the_windows() {
    assert_eq!(Arrival::started(true, true), Arrival::Spawned);
    assert_eq!(Arrival::started(false, true), Arrival::Appeared);
    assert_eq!(Arrival::started(false, false), Arrival::Forwarded);
    assert!(Arrival::Spawned.created());
    assert!(!Arrival::Appeared.created());
}

#[test]
fn only_a_new_instance_is_verified_and_only_an_idle_attach_is_not_delivered() {
    assert_eq!(
        disposition(Arrival::Attached),
        DeliverySemantics::not_delivered()
    );
    assert_eq!(
        disposition(Arrival::Spawned),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(
        disposition(Arrival::Appeared),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(
        disposition(Arrival::Forwarded),
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(
        disposition(Arrival::Unlisted),
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn attaching_and_confirming_the_window_came_forward_is_delivered_verified() {
    let result = conclude(
        &owner(),
        Some(window()),
        &activating(),
        Arrival::Attached,
        |_| Ok(focus_answer(None, true)),
    );

    assert_eq!(result.disposition, DeliverySemantics::delivered_verified());
}

#[test]
fn attaching_to_a_window_already_active_reports_not_delivered() {
    let result = conclude(
        &owner(),
        Some(window()),
        &activating(),
        Arrival::Attached,
        |_| Ok(focus_answer(None, false)),
    );

    assert_eq!(result.disposition, DeliverySemantics::not_delivered());
    assert_eq!(result.disposition.retry(), RetryDisposition::Safe);
}

#[test]
fn attaching_without_a_request_to_activate_dispatches_nothing() {
    let result = conclude(
        &owner(),
        Some(window()),
        &LaunchOptions::default(),
        Arrival::Attached,
        |_| Ok(focus_answer(None, true)),
    );

    assert_eq!(result.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_focus_refused_before_anything_was_sent_leaves_the_attach_not_delivered() {
    let result = conclude(
        &owner(),
        Some(window()),
        &activating(),
        Arrival::Attached,
        |_| {
            Err(AdapterError::new(ErrorCode::ActionFailed, "no focus")
                .with_disposition(DeliverySemantics::not_delivered()))
        },
    );

    assert_eq!(result.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_focus_that_failed_after_an_unknown_amount_was_sent_is_delivered_unverified() {
    let result = conclude(
        &owner(),
        Some(window()),
        &activating(),
        Arrival::Attached,
        |_| Err(AdapterError::new(ErrorCode::ActionFailed, "no focus")),
    );

    assert_eq!(
        result.disposition,
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn a_spawned_application_reports_verified_whether_or_not_it_was_asked_to_activate() {
    for options in [LaunchOptions::default(), activating()] {
        let result = conclude(&owner(), Some(window()), &options, Arrival::Spawned, |_| {
            Ok(focus_answer(None, true))
        });

        assert_eq!(result.disposition, DeliverySemantics::delivered_verified());
    }
}
