use agent_desktop_core::{
    AdapterError, AppInfo, Deadline, DeliverySemantics, ErrorCode, LaunchFacts, LaunchGuidance,
    WindowInfo, system::launch_options::LaunchOptions, system::launch_result::LaunchResult,
};
use std::time::Duration;

use super::launch_window::exact_window;

const MAX_ARGUMENT_COUNT: usize = 256;
const MAX_ENVIRONMENT_COUNT: usize = 256;
const MAX_LAUNCH_TEXT_BYTES: usize = 1024 * 1024;

const ATTACHED_WITHOUT_DISPATCH: LaunchFacts = LaunchFacts {
    attached: true,
    activation_dispatched: false,
    postcondition_observed: false,
};

enum LaunchTarget {
    Existing { pid: i32, process_instance: String },
    New { baseline_instances: Vec<String> },
}

#[cfg(target_os = "macos")]
pub(crate) fn launch_app_impl(
    id: &str,
    options: &LaunchOptions,
    parent_deadline: Deadline,
) -> Result<LaunchResult, AdapterError> {
    validate_app_identifier(id).map_err(before_launch)?;
    validate_launch_options(options).map_err(before_launch)?;
    let deadline = if options.timeout_ms == 0 {
        parent_deadline
    } else {
        parent_deadline.capped(Duration::from_millis(options.timeout_ms))
    };
    ensure_launch_budget(deadline, id).map_err(before_launch)?;
    let initial = matching_apps(id, deadline).map_err(before_launch)?;
    if options.attach_if_running && initial.len() == 1 && !options.activate {
        let app = initial[0].clone();
        let instance = required_instance(&app).map_err(before_launch)?;
        let pid = super::process::process_identity::to_pid_t(app.pid).map_err(before_launch)?;
        let window = exact_window(pid, &instance, deadline).map_err(before_launch)?;
        return Ok(result_from_app(&app, window.report()));
    }
    let target = launch_target(options, initial).map_err(before_launch)?;

    let launched = crate::system::launch_workspace::open(id, options, deadline)?;
    validate_launched_target(&target, &launched).map_err(after_launch)?;
    let window = super::launch_settled::settled_window(launched.0, &launched.1, options, deadline)
        .map_err(after_launch)?;
    let mut facts = target_facts(&target);
    if let LaunchTarget::Existing {
        pid,
        process_instance,
    } = &target
    {
        facts.postcondition_observed =
            super::launch_frontmost::frontmost_is(*pid, process_instance, deadline);
    }
    result_from_launched(&launched, window, id, facts).map_err(after_launch)
}

#[cfg(target_os = "macos")]
pub(super) fn launch_target_gone(pid: i32) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Launched application exited before it presented a window",
    )
    .with_details(serde_json::json!({ "pid": pid, "complete": false }))
    .with_suggestion("Check the application's own launch requirements, then retry.")
}

/// The attaching path reports the display name, so the launching path has to
/// report the same thing for the same field. A window already carries it; the
/// requested identifier is the fallback when there is no window to ask.
#[cfg(target_os = "macos")]
fn launched_display_name(window: Option<&WindowInfo>, id: &str) -> String {
    window
        .map(|window| window.app.clone())
        .unwrap_or_else(|| id.to_owned())
}

#[cfg(target_os = "macos")]
fn result_from_app(app: &AppInfo, window: Option<WindowInfo>) -> LaunchResult {
    let renderer = super::process::process_identity::to_pid_t(app.pid)
        .ok()
        .and_then(crate::system::renderer_kind::detect_chromium);
    LaunchResult {
        app: app.name.clone(),
        pid: app.pid,
        process_instance: app.process_instance.clone(),
        window,
        guidance: LaunchGuidance {
            renderer,
            ..LaunchGuidance::default()
        },
        disposition: ATTACHED_WITHOUT_DISPATCH.disposition(),
    }
}

#[cfg(target_os = "macos")]
fn result_from_launched(
    launched: &(i32, String),
    window: Option<WindowInfo>,
    id: &str,
    facts: LaunchFacts,
) -> Result<LaunchResult, AdapterError> {
    let renderer = crate::system::renderer_kind::detect_chromium(launched.0);
    Ok(LaunchResult {
        app: launched_display_name(window.as_ref(), id),
        pid: agent_desktop_core::ProcessId::try_from(launched.0)
            .map_err(|_| AdapterError::internal("Launched process identifier is out of range"))?,
        process_instance: Some(launched.1.clone()),
        window,
        guidance: LaunchGuidance {
            renderer,
            ..LaunchGuidance::default()
        },
        disposition: facts.disposition(),
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn launch_app_impl(
    _id: &str,
    _options: &LaunchOptions,
    _deadline: Deadline,
) -> Result<LaunchResult, AdapterError> {
    Err(AdapterError::not_supported("launch_app"))
}

#[cfg(target_os = "macos")]
fn launch_target(
    options: &LaunchOptions,
    mut initial: Vec<AppInfo>,
) -> Result<LaunchTarget, AdapterError> {
    if options.attach_if_running {
        if initial.len() > 1 {
            return Err(ambiguous_apps(&initial));
        }
        if let Some(app) = initial.pop() {
            let process_instance = required_instance(&app)?;
            return Ok(LaunchTarget::Existing {
                pid: super::process::process_identity::to_pid_t(app.pid)?,
                process_instance,
            });
        }
    }
    let baseline_instances = initial
        .iter()
        .map(required_instance)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LaunchTarget::New { baseline_instances })
}

#[cfg(target_os = "macos")]
fn target_facts(target: &LaunchTarget) -> LaunchFacts {
    match target {
        LaunchTarget::Existing { .. } => LaunchFacts {
            attached: true,
            activation_dispatched: true,
            postcondition_observed: false,
        },
        LaunchTarget::New { .. } => LaunchFacts {
            attached: false,
            activation_dispatched: false,
            postcondition_observed: true,
        },
    }
}

#[cfg(target_os = "macos")]
fn validate_launched_target(
    target: &LaunchTarget,
    launched: &(i32, String),
) -> Result<(), AdapterError> {
    match target {
        LaunchTarget::Existing {
            pid,
            process_instance,
        } => {
            if launched.0 == *pid && launched.1 == *process_instance {
                Ok(())
            } else {
                Err(AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "NSWorkspace returned a different application while attaching",
                )
                .with_details(serde_json::json!({
                    "expected_pid": pid,
                    "returned_pid": launched.0,
                    "complete": false,
                })))
            }
        }
        LaunchTarget::New { baseline_instances } => {
            if baseline_instances.contains(&launched.1) {
                Err(AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "NSWorkspace reused an existing application during an exact fresh launch",
                )
                .with_details(serde_json::json!({
                    "returned_pid": launched.0,
                    "complete": false,
                })))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn matching_apps(id: &str, deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
    ensure_launch_budget(deadline, id)?;
    let until = deadline;
    crate::system::app::app_inventory::stabilize_apps_until(until, || {
        let inventory = super::app::workspace_apps::list_apps_scoped_until(id, None, until)?;
        if inventory.apps.is_empty() {
            super::app::workspace_apps::require_complete(&inventory.skipped)?;
        }
        Ok(inventory.apps)
    })
}

#[cfg(target_os = "macos")]
fn required_instance(app: &AppInfo) -> Result<String, AdapterError> {
    app.process_instance.clone().ok_or_else(|| {
        AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Running application has no exact process instance token",
        )
        .with_details(serde_json::json!({ "pid": app.pid, "complete": false }))
    })
}

#[cfg(target_os = "macos")]
fn ambiguous_apps(apps: &[AppInfo]) -> AdapterError {
    AdapterError::ambiguous_process_target(
        "More than one application instance matches the launch target",
        &apps.iter().map(|app| app.pid).collect::<Vec<_>>(),
    )
}

#[cfg(target_os = "macos")]
fn ensure_launch_budget(deadline: Deadline, id: &str) -> Result<(), AdapterError> {
    if deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_details(serde_json::json!({ "app_name": id })));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn before_launch(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(target_os = "macos")]
fn validate_app_identifier(id: &str) -> Result<(), AdapterError> {
    let safe_bundle_id = !looks_like_bundle_id(id)
        || id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        });
    if id.is_empty()
        || id.contains("..")
        || id.contains('/')
        || id.chars().any(char::is_control)
        || !safe_bundle_id
    {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Invalid app identifier: use a bare app name or bundle identifier",
        )
        .with_details(serde_json::json!({ "app_name": id }))
        .with_suggestion("Use an app name like 'Safari' or bundle ID like 'com.apple.Safari'."));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_launch_options(options: &LaunchOptions) -> Result<(), AdapterError> {
    if options.cwd.is_some() {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "macOS Launch Services does not support an exact launch working directory",
        )
        .with_suggestion("Remove --cwd, or start the app through a dedicated launcher script"));
    }
    if options.args.len() > MAX_ARGUMENT_COUNT || options.env.len() > MAX_ENVIRONMENT_COUNT {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch argument or environment entry count exceeds the supported limit",
        ));
    }
    let text_bytes = options
        .args
        .iter()
        .map(String::len)
        .chain(
            options
                .env
                .iter()
                .map(|(key, value)| key.len() + value.len()),
        )
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Launch options are too large"))?;
    if text_bytes > MAX_LAUNCH_TEXT_BYTES {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch argument and environment data exceeds one MiB",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn looks_like_bundle_id(id: &str) -> bool {
    id.contains('.') && !id.ends_with(".app") && !id.contains(' ')
}

#[cfg(target_os = "macos")]
fn after_launch(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
