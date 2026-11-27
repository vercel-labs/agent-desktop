use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const KEYBOARD_TOKEN_FILE: &str = "wayland_portal_rd_token";
const POINTER_TOKEN_FILE: &str = "wayland_portal_rd_pointer_token";
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_handle_token(prefix: &str) -> String {
    let seq = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    format!("agent_desktop_{prefix}_{}_{seq}", std::process::id())
}

pub(crate) fn request_path(unique_name: &str, token: &str) -> String {
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")
}

fn token_path(with_screencast: bool) -> Option<PathBuf> {
    let file = if with_screencast {
        POINTER_TOKEN_FILE
    } else {
        KEYBOARD_TOKEN_FILE
    };
    if let Some(root) = env::var_os("AGENT_DESKTOP_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(root).join(file));
    }
    let home = env::var_os("HOME").filter(|value| !value.is_empty())?;
    Some(PathBuf::from(home).join(".agent-desktop").join(file))
}

pub(crate) fn load_restore_token(with_screencast: bool) -> Option<String> {
    let path = token_path(with_screencast)?;
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(crate) fn save_restore_token(with_screencast: bool, token: &str) {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return;
    }
    let Some(path) = token_path(with_screencast) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::write(&path, trimmed).is_ok() {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
}

#[cfg(test)]
#[path = "portal_token_tests.rs"]
mod tests;
