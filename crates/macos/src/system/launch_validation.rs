use agent_desktop_core::{AdapterError, AppInfo, ErrorCode, system::launch_options::LaunchOptions};

const MAX_ARGUMENT_COUNT: usize = 256;
const MAX_ENVIRONMENT_COUNT: usize = 256;
const MAX_LAUNCH_TEXT_BYTES: usize = 1024 * 1024;

#[cfg(target_os = "macos")]
pub(super) fn validate_launch_options(options: &LaunchOptions) -> Result<(), AdapterError> {
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

/// Launch Services hands arguments and environment only to a process it
/// starts. Attaching to the one running instance starts nothing, so a launch
/// that carries either would report success for values the application never
/// received. `--no-attach` starts a fresh instance that does receive them.
#[cfg(target_os = "macos")]
pub(super) fn validate_attach_options(
    options: &LaunchOptions,
    running: &[AppInfo],
) -> Result<(), AdapterError> {
    let carries_launch_data = !options.args.is_empty() || !options.env.is_empty();
    let [app] = running else {
        return Ok(());
    };
    if !options.attach_if_running || !carries_launch_data {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "macOS cannot pass launch arguments or environment to an application that is already running",
    )
    .with_details(serde_json::json!({ "pid": app.pid }))
    .with_suggestion(
        "Remove --arg and --env to attach to the running app, pass --no-attach to start a \
         second instance that receives them, or close the app and launch again",
    ))
}

#[cfg(test)]
#[path = "launch_validation_tests.rs"]
mod tests;
