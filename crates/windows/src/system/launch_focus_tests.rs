use super::*;

const USER: Foreground = Foreground {
    handle: 10,
    pid: 100,
};
const LAUNCHED: u32 = 200;

fn row(pid: u32, parent: u32) -> ProcessRow {
    ProcessRow {
        pid: ProcessId::from(pid),
        name: "app.exe".into(),
        parent,
    }
}

#[test]
fn a_foreground_window_of_the_launched_process_is_stolen_focus() {
    let now = Foreground {
        handle: 20,
        pid: LAUNCHED,
    };
    assert_eq!(judge(USER, Some(now), |pid| pid == LAUNCHED), Look::Stolen);
}

#[test]
fn the_unchanged_foreground_keeps_waiting() {
    assert_eq!(judge(USER, Some(USER), |_| true), Look::Waiting);
}

#[test]
fn a_window_of_a_process_the_launch_started_is_stolen_focus() {
    let rows = [row(300, LAUNCHED), row(400, 300), row(500, 1)];
    let owns = |pid| started_by(LAUNCHED, pid, &rows, |_| true);
    let child = Foreground {
        handle: 20,
        pid: 300,
    };
    let grandchild = Foreground {
        handle: 21,
        pid: 400,
    };
    assert_eq!(judge(USER, Some(child), owns), Look::Stolen);
    assert_eq!(judge(USER, Some(grandchild), owns), Look::Stolen);
}

#[test]
fn a_window_of_an_unrelated_process_is_never_touched() {
    let rows = [row(300, LAUNCHED), row(500, 1)];
    let unrelated = Foreground {
        handle: 30,
        pid: 500,
    };
    let owns = |pid| started_by(LAUNCHED, pid, &rows, |_| true);
    assert_eq!(judge(USER, Some(unrelated), owns), Look::Elsewhere);
    assert_eq!(judge(USER, None, owns), Look::Elsewhere);
}

#[test]
fn a_chain_through_a_process_older_than_the_launch_is_not_counted() {
    let rows = [row(300, LAUNCHED), row(400, 300)];
    let child_of_an_older_process = Foreground {
        handle: 22,
        pid: 400,
    };
    let owns = |pid| started_by(LAUNCHED, pid, &rows, |pid| pid != 300);
    assert_eq!(
        judge(USER, Some(child_of_an_older_process), owns),
        Look::Elsewhere
    );
}

#[test]
fn a_parent_cycle_ends_without_claiming_the_launch() {
    let rows = [row(300, 400), row(400, 300)];
    let now = Foreground {
        handle: 20,
        pid: 300,
    };
    let owns = |pid| started_by(LAUNCHED, pid, &rows, |_| true);
    assert_eq!(judge(USER, Some(now), owns), Look::Elsewhere);
}

#[test]
fn a_background_launch_asks_for_no_activation() {
    let show = startup_show(false);
    assert_eq!(show.flags, 0x1);
    assert_eq!(show.show, 4);
}

#[test]
fn an_activating_launch_keeps_the_default_startup_show() {
    assert_eq!(startup_show(true), StartupShow { flags: 0, show: 0 });
}
