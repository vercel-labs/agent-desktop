use crate::AppError;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const AGENT_DESKTOP_HOME_ENV: &str = "AGENT_DESKTOP_HOME";
const STATE_DIR_NAME: &str = ".agent-desktop";
const LOCK_PROBE_NAME: &str = "refstore.lock";

pub(crate) struct StateRootInputs {
    pub override_active: bool,
    pub env_value: Option<PathBuf>,
    pub home_fallback: Option<PathBuf>,
    pub temp_fallback: Option<PathBuf>,
}

/// Binary-level preflight: validates `AGENT_DESKTOP_HOME` without touching
/// `HOME`/`USERPROFILE`. A no-op when the variable is unset, so legacy
/// behavior (surfacing a missing-HOME error later, at first state touch)
/// stays byte-identical to today.
pub fn validate_state_root_env() -> Result<(), AppError> {
    let override_active = crate::refs::home_override_active();
    let env_value = std::env::var_os(AGENT_DESKTOP_HOME_ENV).map(PathBuf::from);
    validate_state_root_env_value(override_active, env_value, is_owned_by_current_user)
}

pub(crate) fn validate_state_root_env_value(
    override_active: bool,
    env_value: Option<PathBuf>,
    owned_by_current_user: impl Fn(&std::fs::Metadata) -> bool,
) -> Result<(), AppError> {
    if override_active {
        return Ok(());
    }
    match env_value {
        Some(path) => validate_env_root(&path, &owned_by_current_user),
        None => Ok(()),
    }
}

/// Validates and prepares an operator-configured private output directory
/// (such as `mcp --screenshot-dir`), creating it with private permissions when
/// missing and rejecting relative paths, parent traversal, symlinks, foreign
/// owners, or group/world-accessible directories.
pub fn ensure_private_output_dir(path: &Path) -> Result<(), AppError> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(AppError::invalid_input_with_suggestion(
            format!(
                "Output directory must be an absolute path, got '{}'",
                path.display()
            ),
            "Pass an absolute directory path that you own.",
        ));
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(AppError::invalid_input_with_suggestion(
            format!(
                "Output directory must not contain '..': '{}'",
                path.display()
            ),
            "Pass a canonical absolute directory path without '..' segments.",
        ));
    }
    crate::storage::private_file_parent::ensure_private(path).map_err(|err| {
        AppError::invalid_input_with_suggestion(
            format!(
                "Output directory '{}' is not a private user-owned directory: {err}",
                path.display()
            ),
            "Choose a private (0700) directory you own, not a shared or symlinked path.",
        )
    })
}

pub(crate) fn resolve_configured_state_root() -> Result<PathBuf, AppError> {
    let inputs = StateRootInputs {
        override_active: crate::refs::home_override_active(),
        env_value: std::env::var_os(AGENT_DESKTOP_HOME_ENV).map(PathBuf::from),
        home_fallback: crate::refs::home_dir(),
        temp_fallback: default_private_runtime_root(),
    };
    resolve_state_root_with_fallback(inputs, is_owned_by_current_user, is_sandbox_blocked)
}

#[cfg(test)]
pub(crate) fn resolve_state_root(
    override_active: bool,
    env_value: Option<PathBuf>,
    home_fallback: Option<PathBuf>,
    owned_by_current_user: impl Fn(&std::fs::Metadata) -> bool,
) -> Result<PathBuf, AppError> {
    let inputs = StateRootInputs {
        override_active,
        env_value,
        home_fallback,
        temp_fallback: None,
    };
    resolve_state_root_with_fallback(inputs, owned_by_current_user, |_| false)
}

pub(crate) fn resolve_state_root_with_fallback(
    inputs: StateRootInputs,
    owned_by_current_user: impl Fn(&std::fs::Metadata) -> bool,
    sandbox_blocked: impl Fn(&Path) -> bool,
) -> Result<PathBuf, AppError> {
    if !inputs.override_active
        && let Some(env_path) = inputs.env_value
    {
        validate_env_root(&env_path, &owned_by_current_user)?;
        return Ok(env_path);
    }
    let home = inputs
        .home_fallback
        .ok_or_else(|| AppError::Internal("HOME directory not found".into()))?;
    let primary = home.join(STATE_DIR_NAME);
    if !inputs.override_active
        && sandbox_blocked(&primary)
        && let Some(fallback) = inputs.temp_fallback
        && prepare_private_state_root(&fallback).is_ok()
    {
        return Ok(fallback);
    }
    Ok(primary)
}

fn is_sandbox_blocked(primary: &Path) -> bool {
    let Some(parent) = primary.parent() else {
        return false;
    };
    let Ok(parent_meta) = std::fs::symlink_metadata(parent) else {
        return false;
    };
    if parent_meta.file_type().is_symlink() || !parent_meta.is_dir() {
        return false;
    }
    if let Err(err) = crate::storage::private_file_parent::ensure_private(primary) {
        return is_os_sandbox_denial(&err)
            && matches!(
                std::fs::symlink_metadata(primary),
                Err(meta_err) if meta_err.kind() == ErrorKind::NotFound
            );
    }
    let probe = primary.join(LOCK_PROBE_NAME);
    match std::fs::symlink_metadata(&probe) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => return false,
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(_) => return false,
    }
    match crate::storage::private_file::open_private_lock(&probe, true) {
        Ok(_) => false,
        Err(err) => is_os_sandbox_denial(&err),
    }
}

fn is_os_sandbox_denial(err: &std::io::Error) -> bool {
    err.raw_os_error().is_some()
        && matches!(
            err.kind(),
            ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
        )
}

fn prepare_private_state_root(root: &Path) -> std::io::Result<()> {
    crate::storage::private_file_parent::ensure_private(root)?;
    let probe = root.join(LOCK_PROBE_NAME);
    let _ = crate::storage::private_file::open_private_lock(&probe, true)?;
    Ok(())
}

#[cfg(unix)]
fn default_private_runtime_root() -> Option<PathBuf> {
    for var in ["XDG_RUNTIME_DIR", "TMPDIR"] {
        let Some(raw) = std::env::var_os(var) else {
            continue;
        };
        let base = PathBuf::from(raw);
        if is_private_runtime_base(&base) {
            return Some(base.join("agent-desktop"));
        }
    }
    None
}

#[cfg(unix)]
pub(super) fn is_private_runtime_base(base: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    if !base.is_absolute()
        || base
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    let Ok(meta) = std::fs::symlink_metadata(base) else {
        return false;
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return false;
    }
    meta.uid() == unsafe { libc::geteuid() } && (meta.mode() & 0o077 == 0)
}

#[cfg(not(unix))]
fn default_private_runtime_root() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    if !base.is_absolute()
        || base
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    let meta = std::fs::symlink_metadata(&base).ok()?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return None;
    }
    Some(base.join("agent-desktop"))
}

fn validate_env_root(
    path: &Path,
    owned_by_current_user: &dyn Fn(&std::fs::Metadata) -> bool,
) -> Result<(), AppError> {
    if path.as_os_str().is_empty() {
        return Err(AppError::invalid_input_with_suggestion(
            "AGENT_DESKTOP_HOME must not be empty",
            "Unset the variable or set it to an absolute directory path.",
        ));
    }
    if !path.is_absolute() {
        return Err(AppError::invalid_input_with_suggestion(
            format!(
                "AGENT_DESKTOP_HOME must be an absolute path, got '{}'",
                path.display()
            ),
            "Set AGENT_DESKTOP_HOME to an absolute directory path.",
        ));
    }
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(AppError::invalid_input_with_suggestion(
                    format!(
                        "AGENT_DESKTOP_HOME must not be a symlink: '{}'",
                        path.display()
                    ),
                    "Point AGENT_DESKTOP_HOME at a real directory, not a symlink.",
                ));
            }
            if !meta.is_dir() {
                return Err(AppError::invalid_input_with_suggestion(
                    format!(
                        "AGENT_DESKTOP_HOME must be a directory: '{}'",
                        path.display()
                    ),
                    "Remove the file at this path or choose a different AGENT_DESKTOP_HOME.",
                ));
            }
            if !owned_by_current_user(&meta) {
                return Err(AppError::invalid_input_with_suggestion(
                    format!(
                        "AGENT_DESKTOP_HOME is owned by a different user: '{}'",
                        path.display()
                    ),
                    "Choose a directory you own for AGENT_DESKTOP_HOME.",
                ));
            }
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::invalid_input_with_suggestion(
            format!(
                "Cannot access AGENT_DESKTOP_HOME '{}': {err}",
                path.display()
            ),
            "Check permissions on the parent directory or choose a different path.",
        )),
    }
}

#[cfg(unix)]
pub(crate) fn is_owned_by_current_user(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    meta.uid() == unsafe { libc::getuid() }
}

#[cfg(not(unix))]
pub(crate) fn is_owned_by_current_user(_meta: &std::fs::Metadata) -> bool {
    true
}

#[cfg(test)]
pub(super) fn lock_read_only_private_dir_for_test(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return false;
        }
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o500)).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

#[cfg(test)]
pub(super) fn unlock_read_only_private_dir_for_test(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

#[cfg(test)]
pub(super) fn create_user_symlink_for_test(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } == 0 {
            return false;
        }
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link);
        false
    }
}

#[cfg(test)]
pub(super) fn is_world_writable_runtime_base_rejected_for_test(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o777)).is_err() {
            return false;
        }
        !is_private_runtime_base(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        true
    }
}

#[cfg(test)]
#[path = "state_root_tests.rs"]
mod tests;
