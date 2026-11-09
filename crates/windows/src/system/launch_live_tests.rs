use super::super::*;
use super::{
    KillOnDrop, copy_system_exe, deadline, matching_pids, scratch_dir, start_named_windowless,
};
use agent_desktop_core::Deadline;

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
        Deadline::after(1_000).expect("deadline"),
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
        deadline(10_000),
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
        deadline(10_000),
    )
    .expect("a windowless process is a successful launch");
    let _guard = KillOnDrop(result.pid);
    assert!(
        result.window.is_none(),
        "timeout 0 observes once and reports no window rather than polling: {result:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
