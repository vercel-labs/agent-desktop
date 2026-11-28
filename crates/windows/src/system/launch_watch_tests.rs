use super::*;

fn row(pid: u32, parent: u32) -> ProcessRow {
    ProcessRow {
        pid: ProcessId::from(pid),
        parent: ProcessId::from(parent),
        name: "probe.exe".into(),
    }
}

#[test]
fn every_sight_moves_a_polling_watch_to_its_one_end_state() {
    assert_eq!(Watch::Polling.advance(Sight::NoWindow), Watch::Polling);
    assert_eq!(Watch::Polling.advance(Sight::Window), Watch::WindowSeen);
    assert_eq!(
        Watch::Polling.advance(Sight::NothingLeftRunning),
        Watch::LauncherFinished
    );
    assert_eq!(Watch::Polling.advance(Sight::OutOfTime), Watch::TimedOut);
}

#[test]
fn an_ended_watch_ignores_every_later_sight() {
    for ended in [Watch::WindowSeen, Watch::LauncherFinished, Watch::TimedOut] {
        for sight in [
            Sight::Window,
            Sight::NoWindow,
            Sight::NothingLeftRunning,
            Sight::OutOfTime,
        ] {
            assert_eq!(ended.advance(sight), ended);
        }
    }
}

#[test]
fn zero_wait_never_polls_after_first_observation() {
    assert!(!should_poll_after_first_observation(0));
    assert!(should_poll_after_first_observation(1));
}

#[test]
fn descendants_follow_parent_links_through_started_processes() {
    let rows = [
        row(10, 1),
        row(11, 10),
        row(12, 11),
        row(20, 1),
        row(21, 20),
    ];

    let started = descendants(&rows, ProcessId::from(10), 10_000, |_| Some(10_000));

    assert_eq!(started, vec![ProcessId::from(11), ProcessId::from(12)]);
}

#[test]
fn descendants_ignore_preexisting_processes_on_a_reused_parent_pid() {
    let rows = [row(10, 1), row(11, 10), row(12, 10)];

    let started = descendants(&rows, ProcessId::from(10), 20_000, |pid| match pid {
        11 => Some(1_000),
        12 => Some(20_050),
        _ => None,
    });

    assert_eq!(started, vec![ProcessId::from(12)]);
}

#[test]
fn a_process_with_nothing_started_has_no_descendants() {
    let rows = [row(10, 1), row(20, 1)];

    assert!(descendants(&rows, ProcessId::from(10), 10_000, |_| Some(10_000)).is_empty());
}

#[test]
fn a_parent_cycle_ends_instead_of_looping() {
    let rows = [row(10, 11), row(11, 10), row(0, 0)];

    assert_eq!(
        descendants(&rows, ProcessId::from(10), 10_000, |_| Some(10_000)),
        vec![ProcessId::from(11)]
    );
    assert!(descendants(&rows, ProcessId::from(0), 10_000, |_| Some(10_000)).is_empty());
}

#[test]
fn a_forwardable_launch_keeps_looking_for_the_forwarded_window_after_its_launcher_exits() {
    let exited = Instant::now();

    assert!(forward_search_over(false, exited, exited));
    assert!(!forward_search_over(true, exited, exited));
    assert!(forward_search_over(true, exited, exited + FORWARD_GRACE));
}
