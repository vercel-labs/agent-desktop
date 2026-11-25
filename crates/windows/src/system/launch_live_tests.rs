use super::super::*;
use super::{KillOnDrop, copy_system_exe, matching_pids, scratch_dir, start_named_windowless};
use crate::system::test_support::test_time::test_lease;

#[test]
fn attach_true_windowless_does_not_launch_second_process() {
    let dir = scratch_dir("windowless");
    let probe = copy_system_exe("ping.exe", &dir, "launchprobe_ping.exe");
    let image = "launchprobe_ping.exe";
    let _ping = start_named_windowless(&probe, &["-n", "60", "127.0.0.1"]);
    let running = matching_pids(image);
    assert_eq!(running.len(), 1);
    let result = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            attach_if_running: true,
            timeout_ms: 300,
            ..Default::default()
        },
        test_lease(1_000),
    )
    .expect("attaching to a windowless process is a successful launch, not a failure");
    assert!(
        result.window.is_none(),
        "a windowless process reports no window rather than an error: {result:?}"
    );
    assert_eq!(
        result.pid, running[0],
        "attach returns the already-running process rather than starting one"
    );
    assert_eq!(
        matching_pids(image).len(),
        1,
        "attach must not spawn a second process"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cwd_is_honored_for_create_process() {
    let temp = scratch_dir("cwd");
    let probe = copy_system_exe("cmd.exe", &temp, "launchprobe_cwd.exe");
    let marker = temp.join("cwd.txt");
    let options = LaunchOptions {
        args: vec!["/C".into(), format!("cd > \"{}\"", marker.display())],
        cwd: Some(temp.clone()),
        attach_if_running: false,
        timeout_ms: 0,
        ..Default::default()
    };
    let result = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &options,
        test_lease(10_000),
    )
    .expect("a windowless probe is a successful launch");
    let _guard = KillOnDrop(result.pid);
    assert!(
        result.window.is_none(),
        "the probe presents no window: {result:?}"
    );
    let written = poll_read_marker(&marker, Deadline::after(5_000).expect("deadline"));
    let normalized_written = written.trim().trim_end_matches('\\').to_ascii_lowercase();
    let normalized_temp = temp
        .to_string_lossy()
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    assert_eq!(normalized_written, normalized_temp);
    let _ = std::fs::remove_dir_all(&temp);
}

/// The marker is written by a child shell redirecting `cd` output, a step
/// that lands strictly after `CreateProcessW` returns to this test. Reading
/// it once, immediately, races that write; polling under a deadline lets the
/// child finish without turning a slow scheduler into a false failure.
fn poll_read_marker(marker: &std::path::Path, deadline: Deadline) -> String {
    loop {
        if let Ok(contents) = std::fs::read_to_string(marker) {
            if !contents.trim().is_empty() {
                return contents;
            }
        }
        if deadline.remaining().is_zero() {
            return std::fs::read_to_string(marker).expect("cwd marker");
        }
        std::thread::sleep(std::time::Duration::from_millis(50).min(deadline.remaining()));
    }
}

#[test]
fn timeout_zero_checks_once_for_process_without_window() {
    let dir = scratch_dir("timeout0");
    let probe = copy_system_exe("ping.exe", &dir, "launchprobe_timeout.exe");
    let result = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            args: vec!["-n".into(), "30".into(), "127.0.0.1".into()],
            attach_if_running: false,
            timeout_ms: 0,
            ..Default::default()
        },
        test_lease(10_000),
    )
    .expect("a windowless process is a successful launch");
    let _guard = KillOnDrop(result.pid);
    assert!(
        result.window.is_none(),
        "timeout 0 observes once and reports no window rather than polling: {result:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn two_running_probes(label: &str) -> (std::path::PathBuf, [KillOnDrop; 2]) {
    let dir = scratch_dir(label);
    let image = format!("launchprobe_{label}.exe");
    let probe = copy_system_exe("ping.exe", &dir, &image);
    let first = start_named_windowless(&probe, &["-n", "60", "127.0.0.1"]);
    let second = start_named_windowless(&probe, &["-n", "60", "127.0.0.1"]);
    assert_eq!(matching_pids(&image).len(), 2);
    (dir, [first, second])
}

#[test]
fn a_launch_with_arguments_spawns_even_when_several_instances_run() {
    let (dir, _running) = two_running_probes("two_args");
    let probe = dir.join("launchprobe_two_args.exe");

    let result = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            args: vec!["-n".into(), "30".into(), "127.0.0.1".into()],
            attach_if_running: true,
            timeout_ms: 0,
            ..Default::default()
        },
        test_lease(10_000),
    )
    .expect("arguments are forwarded by a spawn, whatever instances run");
    let _guard = KillOnDrop(result.pid);

    assert_eq!(matching_pids("launchprobe_two_args.exe").len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_bare_launch_is_ambiguous_when_several_instances_run() {
    let (dir, _running) = two_running_probes("two_bare");
    let probe = dir.join("launchprobe_two_bare.exe");

    let error = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            attach_if_running: true,
            timeout_ms: 0,
            ..Default::default()
        },
        test_lease(10_000),
    )
    .expect_err("a bare attach has to choose one of two instances");

    assert_eq!(error.code, agent_desktop_core::ErrorCode::AmbiguousTarget);
    assert_eq!(matching_pids("launchprobe_two_bare.exe").len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_launch_with_arguments_spawns_when_one_instance_runs() {
    let dir = scratch_dir("one-args");
    let probe = copy_system_exe("ping.exe", &dir, "launchprobe_one.exe");
    let _running = start_named_windowless(&probe, &["-n", "60", "127.0.0.1"]);

    let result = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            args: vec!["-n".into(), "30".into(), "127.0.0.1".into()],
            attach_if_running: true,
            timeout_ms: 0,
            ..Default::default()
        },
        test_lease(10_000),
    )
    .expect("arguments are forwarded by a spawn");
    let _guard = KillOnDrop(result.pid);

    assert_eq!(matching_pids("launchprobe_one.exe").len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
