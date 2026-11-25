use super::launch_watch::{Origin, Seen, Sighting, Watch};
use super::*;
use agent_desktop_core::{DeliverySemantics, LaunchArrival, ProcessId, WindowInfo};

#[test]
fn attaching_dispatches_nothing_so_it_reports_not_delivered() {
    let result = launch_result(reported(LaunchArrival::Attached, None));

    assert_eq!(result.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_created_process_with_a_read_back_token_reports_delivered_verified() {
    let result = launch_result(reported(LaunchArrival::Spawned, None));

    assert_eq!(result.disposition, DeliverySemantics::delivered_verified());
}

fn reported(arrival: LaunchArrival, suggestion: Option<&'static str>) -> Reported<'static> {
    Reported {
        id: "notepad.exe",
        pid: ProcessId::new(7),
        process_instance: "t".into(),
        window: None,
        arrival,
        suggestion,
    }
}

#[test]
fn a_launch_that_ended_without_a_window_is_never_verified() {
    for end in [Watch::LauncherFinished, Watch::TimedOut] {
        let result = spawned_result(
            "control.exe",
            ProcessId::new(7),
            "t".into(),
            Sighting { seen: None, end },
        );

        assert_eq!(
            result.disposition,
            DeliverySemantics::delivered_unverified()
        );
        assert_eq!(result.pid, ProcessId::new(7));
        assert!(result.window.is_none());
        assert!(
            result
                .guidance
                .suggestion
                .as_deref()
                .is_some_and(|text| text.contains("list-windows")),
            "the suggestion must name the step that finds the real owner"
        );
    }
}

#[test]
fn a_new_window_of_a_running_instance_is_a_forward_that_names_that_instance() {
    let window = WindowInfo {
        id: "w-9".into(),
        title: "Folder".into(),
        app: "explorer".into(),
        pid: ProcessId::new(500),
        process_instance: Some("shell-token".into()),
        bounds: None,
        state: Default::default(),
    };

    let result = spawned_result(
        "explorer.exe",
        ProcessId::new(7),
        "launcher-token".into(),
        Sighting {
            seen: Some(Seen {
                window,
                origin: Origin::Resident,
            }),
            end: Watch::WindowSeen,
        },
    );

    assert_eq!(result.pid, ProcessId::new(500));
    assert_eq!(
        result.disposition,
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn a_window_of_a_started_process_names_that_process_as_the_owner() {
    let window = WindowInfo {
        id: "w-9".into(),
        title: "Calculator".into(),
        app: "calc".into(),
        pid: ProcessId::new(99),
        process_instance: Some("child-token".into()),
        bounds: None,
        state: Default::default(),
    };

    let result = spawned_result(
        "calc.exe",
        ProcessId::new(7),
        "stub-token".into(),
        Sighting {
            seen: Some(Seen {
                window,
                origin: Origin::Started,
            }),
            end: Watch::WindowSeen,
        },
    );

    assert_eq!(result.pid, ProcessId::new(99));
    assert_eq!(result.process_instance.as_deref(), Some("child-token"));
    assert_eq!(result.disposition, DeliverySemantics::delivered_verified());
}
