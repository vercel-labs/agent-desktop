use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ProcessId, WindowFilter, WindowInfo,
    system::launch_options::LaunchOptions,
};

use super::super::window::window_ops::list_windows_live;

/// The window a launch actually caused, or `None`. A process that presents no
/// window is a fact the caller reads from `LaunchResult.window`, not a failure:
/// a background or windowless process is a legitimate launch outcome, and
/// erroring on it made every such launch spend its whole timeout first.
pub(super) fn observe_window(
    pid: ProcessId,
    process_instance: &str,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    let mut poll_interval = Duration::from_millis(50);
    loop {
        match observe_window_once(pid, process_instance, deadline) {
            Ok(Some(window)) => return Ok(Some(window)),
            Ok(None) => {}
            Err(error) => return Err(error),
        }
        if !should_poll_after_first_observation(options.timeout_ms)
            || deadline.remaining().is_zero()
        {
            return Ok(None);
        }
        let remaining = deadline.remaining();
        std::thread::sleep(poll_interval.min(remaining));
        poll_interval = (poll_interval * 3 / 2).min(Duration::from_millis(250));
    }
}

pub(super) fn should_poll_after_first_observation(timeout_ms: u64) -> bool {
    timeout_ms > 0
}

/// One window observation, tolerant of the inventory's mid-walk identity race.
///
/// `list_windows_live` now absorbs that race itself (the shared listing-retry
/// budget every caller on this path shares), retrying internally before ever
/// returning `WINDOW_NOT_FOUND` — so this function no longer re-reads the
/// inventory itself. A launch is exactly the moment that race is most
/// reachable, since a process was just created or terminated, but a race that
/// still survives the shared budget, and a budget that runs out entirely
/// (`TIMEOUT`), both say the desktop could not be read coherently in time —
/// not that this launch failed — so both are reported as no window observed.
/// Every other failure still propagates: a target whose process instance
/// changed underneath the launch is a real answer, not a race.
pub(super) fn observe_window_once(
    pid: ProcessId,
    process_instance: &str,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    match exact_window(pid, process_instance, deadline) {
        Ok(window) => Ok(window),
        Err(error) if is_unobservable_within_budget(&error) => Ok(None),
        Err(error) => Err(super::after_launch(error)),
    }
}

fn is_unobservable_within_budget(error: &AdapterError) -> bool {
    matches!(error.code, ErrorCode::WindowNotFound | ErrorCode::Timeout)
}

fn exact_window(
    pid: ProcessId,
    process_instance: &str,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    let windows = list_windows_live(
        &WindowFilter {
            focused_only: false,
            app: None,
        },
        deadline,
    )?;
    let Some(window) = windows.into_iter().find(|window| window.pid == pid) else {
        return Ok(None);
    };
    if window.process_instance.as_deref() != Some(process_instance) {
        return Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Application process instance changed while waiting for its window",
        )
        .with_details(serde_json::json!({ "pid": pid, "complete": false })));
    }
    Ok(Some(window))
}
