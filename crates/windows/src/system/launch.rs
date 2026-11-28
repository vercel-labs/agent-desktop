use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionLease, LaunchArrival,
    LaunchGuidance, ProcessId, RendererKind, WindowInfo, system::launch_options::LaunchOptions,
    system::launch_result::LaunchResult,
};
#[cfg(test)]
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

#[cfg(test)]
use self::launch_attach::matching_processes;
use self::launch_attach::prepare;
use self::launch_watch::{Origin, Resident, Seen, Sighting, Target, Watch, observe_window};
use super::app_ops::ProcessRow;
use super::launch_path::{
    child_environment_block, image_file_name, launch_identifier, validate_launch_options,
};
#[cfg(target_os = "windows")]
use super::launch_path::{to_wide, to_wide_str};
use super::process_identity;
#[cfg(target_os = "windows")]
use super::runtime::hresult::win32_last_error;

const STARTF_USESHOWWINDOW: u32 = 0x1;
const SW_SHOWNOACTIVATE: u16 = 4;

/// The `dwFlags` and `wShowWindow` fields of the child's `STARTUPINFOW`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StartupShow {
    flags: u32,
    show: u16,
}

/// A launch without `--activate` asks for no activation. An application that
/// ignores the hint is handled by the front guard.
fn startup_show(activate: bool) -> StartupShow {
    if activate {
        return StartupShow { flags: 0, show: 0 };
    }
    StartupShow {
        flags: STARTF_USESHOWWINDOW,
        show: SW_SHOWNOACTIVATE,
    }
}

pub(crate) fn launch_app_impl(
    id: &str,
    options: &LaunchOptions,
    lease: InteractionLease,
) -> Result<LaunchResult, AdapterError> {
    let parent_deadline = lease.deadline();
    let id = &launch_identifier(id).map_err(before_launch)?;
    validate_launch_options(options).map_err(before_launch)?;
    let deadline = if options.timeout_ms == 0 {
        parent_deadline
    } else {
        parent_deadline.capped(Duration::from_millis(options.timeout_ms))
    };
    deadline.ensure_remaining().map_err(before_launch)?;
    let (executable, attached) = prepare(id, options, deadline).map_err(before_launch)?;
    if let Some(attached) = attached {
        return Ok(attached);
    }
    let known_windows = launch_watch::window_ids(deadline);
    let (pid, token, started_ms) = create_process(&executable, options, deadline)?;
    let target = Target {
        pid,
        process_instance: &token,
        started_ms,
        resident: known_windows.map(|known_windows| Resident {
            image: executable.clone(),
            known_windows,
        }),
    };
    let sighting = observe_window(&target, options, deadline)?;
    drop(lease);
    Ok(spawned_result(id, pid, token, sighting))
}

const FINISHED_WITHOUT_WINDOW: &str = "The launched process exited and no window of it or of a process it started appeared. It may have handed its work to another process. Run 'list-windows' to find it.";

const RUNNING_WITHOUT_WINDOW: &str = "The launched process is running and showed no window within the wait. Run 'list-windows' to see whether one opened later.";

/// The one place a finished watch becomes a result. A window owned by the
/// launched process is `Spawned`; one owned by a process it started is
/// `Appeared` and names that owner. A window the launch caused in an instance
/// of the program that was already running is `Forwarded`: the request
/// arrived, and the arrival was read but not attributed beyond the program
/// and the new window. Without a window the process was created
/// and nothing of the launch was observed, so the result says what ended the
/// wait and claims no verification.
fn spawned_result(
    id: &str,
    pid: ProcessId,
    process_instance: String,
    sighting: Sighting,
) -> LaunchResult {
    let Sighting { seen, end } = sighting;
    match seen {
        Some(Seen { window, origin }) => {
            let owner = window.pid;
            let instance = window.process_instance.clone().unwrap_or(process_instance);
            let arrival = match origin {
                Origin::Launched => LaunchArrival::Spawned,
                Origin::Started => LaunchArrival::Appeared,
                Origin::Resident => LaunchArrival::Forwarded,
            };
            launch_result(Reported {
                id,
                pid: owner,
                process_instance: instance,
                window: Some(window),
                arrival,
                suggestion: None,
            })
        }
        None => launch_result(Reported {
            id,
            pid,
            process_instance,
            window: None,
            arrival: LaunchArrival::Unobserved,
            suggestion: Some(if end == Watch::LauncherFinished {
                FINISHED_WITHOUT_WINDOW
            } else {
                RUNNING_WITHOUT_WINDOW
            }),
        }),
    }
}

pub(super) struct Reported<'a> {
    pub(super) id: &'a str,
    pub(super) pid: ProcessId,
    pub(super) process_instance: String,
    pub(super) window: Option<WindowInfo>,
    pub(super) arrival: LaunchArrival,
    pub(super) suggestion: Option<&'static str>,
}

fn launch_result(reported: Reported<'_>) -> LaunchResult {
    let Reported {
        id,
        pid,
        process_instance,
        window,
        arrival,
        suggestion,
    } = reported;
    let renderer = window.as_ref().and_then(detect_renderer_from_window);
    LaunchResult {
        app: window
            .as_ref()
            .map(|window| window.app.clone())
            .unwrap_or_else(|| image_file_name(id).to_string()),
        pid,
        process_instance: Some(process_instance),
        window,
        guidance: LaunchGuidance {
            renderer,
            suggestion: suggestion.map(str::to_owned),
            ..LaunchGuidance::default()
        },
        disposition: arrival.facts().disposition(),
    }
}

#[cfg(target_os = "windows")]
fn detect_renderer_from_window(window: &WindowInfo) -> Option<RendererKind> {
    const CHROMIUM_WINDOW_CLASS: &str = "Chrome_WidgetWin_1";
    let handle = super::window::window_ops::parse_handle(&window.id);
    if handle.is_null() {
        return None;
    }
    super::window::window_ops::window_class_name(handle)
        .filter(|class| class == CHROMIUM_WINDOW_CLASS)
        .map(|_| RendererKind::Chromium)
}

#[cfg(not(target_os = "windows"))]
fn detect_renderer_from_window(_window: &WindowInfo) -> Option<RendererKind> {
    None
}

/// Refuses a relative working directory before it reaches `CreateProcessW`.
///
/// The child's current directory sits early in the default DLL search order,
/// so it picks which copy of a dependency loads; a relative path resolves
/// against whatever directory this process happens to hold, not the caller's.
#[cfg(target_os = "windows")]
fn absolute_cwd(cwd: &Path) -> Result<&Path, AdapterError> {
    if cwd.is_absolute() {
        return Ok(cwd);
    }
    Err(before_launch(AdapterError::new(
        ErrorCode::InvalidArgs,
        "The launch working directory must be an absolute path",
    )))
}

#[cfg(target_os = "windows")]
fn create_process(
    executable: &Path,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<(ProcessId, String, u64), AdapterError> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CREATE_UNICODE_ENVIRONMENT, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
    };

    deadline.ensure_remaining().map_err(before_launch)?;
    let app_wide = to_wide(executable).map_err(before_launch)?;
    let command_line = command_line_for(executable, &options.args).map_err(before_launch)?;
    let mut command_wide = to_wide_str(&command_line).map_err(before_launch)?;
    let cwd_wide = match &options.cwd {
        Some(cwd) => Some(to_wide(absolute_cwd(cwd)?).map_err(before_launch)?),
        None => None,
    };
    let env_block = child_environment_block(
        &options.env,
        crate::system::interaction_lease::INTERACTION_LEASE_HANDLE_ENV,
    )
    .map_err(before_launch)?;
    let show = startup_show(options.activate);
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        dwFlags: show.flags,
        wShowWindow: show.show,
        ..Default::default()
    };
    let mut information = PROCESS_INFORMATION::default();
    let flags = if env_block.is_some() {
        CREATE_UNICODE_ENVIRONMENT
    } else {
        0
    };
    let env_ptr = env_block
        .as_ref()
        .map(|block| block.as_ptr().cast())
        .unwrap_or(std::ptr::null());
    let cwd_ptr = cwd_wide
        .as_ref()
        .map(|wide| wide.as_ptr())
        .unwrap_or(std::ptr::null());
    deadline.ensure_remaining().map_err(before_launch)?;
    let ok = unsafe {
        CreateProcessW(
            app_wide.as_ptr(),
            command_wide.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            flags,
            env_ptr,
            cwd_ptr,
            &startup,
            &mut information,
        )
    };
    if ok == 0 {
        return Err(before_launch(win32_last_error(
            "CreateProcessW failed to start the application",
        )));
    }
    let pid = ProcessId::from(information.dwProcessId);
    unsafe {
        CloseHandle(information.hThread);
        CloseHandle(information.hProcess);
    }
    let identity = process_identity::ProcessIdentity::capture(pid)
        .map_err(after_launch)?
        .ok_or_else(|| {
            after_launch(
                AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "Created process has no exact process instance token",
                )
                .with_details(serde_json::json!({ "pid": pid, "complete": false })),
            )
        })?;
    Ok((pid, identity.token(), identity.started_unix_ms()))
}

#[cfg(not(target_os = "windows"))]
fn create_process(
    _executable: &Path,
    _options: &LaunchOptions,
    _deadline: Deadline,
) -> Result<(ProcessId, String, u64), AdapterError> {
    Err(AdapterError::not_supported("launch_app"))
}

fn command_line_for(executable: &Path, args: &[String]) -> Result<String, AdapterError> {
    let mut line = quote_arg(&executable.to_string_lossy());
    for arg in args {
        line.push(' ');
        line.push_str(&quote_arg(arg));
    }
    Ok(line)
}

/// Encodes one argument for the command line the child parses back apart.
///
/// The trailing backslash run is the case that silently corrupts: backslashes
/// are literal except immediately before a quote, so a value ending in one
/// would escape the closing quote this appends, leaving the quoted region open
/// and swallowing every later argument into it. That run is therefore doubled.
/// An embedded quote stays doubled rather than backslash-escaped: that is a
/// valid encoding for the consecutive-quote rule and the form `cmd` accepts,
/// which backslash-escaping is not.
fn quote_arg(value: &str) -> String {
    if !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c == '"') {
        return value.to_string();
    }
    let mut out = String::from("\"");
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    let trailing = value.len() - value.trim_end_matches('\\').len();
    for _ in 0..trailing {
        out.push('\\');
    }
    out.push('"');
    out
}

/// Pass-through kept here, under this module's own name, purely so this
/// module's own tests (`launch_tests.rs`) can call it unqualified through
/// their `use super::*;`; the merge itself lives in `launch_path` beside
/// every other input-shape rule a launch enforces, and `create_process`
/// reaches it through `child_environment_block` rather than this name. Only
/// that test caller remains, so this exists for `#[cfg(test)]` builds alone.
#[cfg(test)]
fn environment_block(overrides: &BTreeMap<String, String>) -> Result<Vec<u16>, AdapterError> {
    super::launch_path::environment_block(overrides)
}

fn ambiguous_apps(matches: &[ProcessRow]) -> AdapterError {
    AdapterError::ambiguous_process_target(
        "More than one application instance matches the launch target",
        &matches.iter().map(|row| row.pid).collect::<Vec<_>>(),
    )
}

fn before_launch(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

fn after_launch(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "launch_result_tests.rs"]
mod result_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "launch_lease_isolation_tests.rs"]
mod launch_lease_isolation_tests;

#[path = "launch_attach.rs"]
pub(crate) mod launch_attach;

#[path = "launch_watch.rs"]
mod launch_watch;
