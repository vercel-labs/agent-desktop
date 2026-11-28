use agent_desktop_core::{
    AdapterError, AppInfo, Deadline, ErrorCode, ProcessId, ProcessIdentity, WindowInfo,
};

use super::surface_inventory::windows_of;
use crate::system::window::window_enum::{EnumeratedWindow, WindowHandle};
use crate::system::window::window_identity::live_window_title;

/// The popover a snapshot, a surface read or a stored ref means.
///
/// A named id must still be one of the process's open popovers: a handle the
/// process no longer presents that way is refused, never followed. With no id
/// there must be exactly one open popover. Several open popovers are refused
/// with their ids rather than resolved to the first, because the caller named
/// none and picking one would land on a popover it never read.
pub(crate) fn resolve_popover(
    pid: ProcessId,
    named: Option<&str>,
    deadline: Deadline,
) -> Result<EnumeratedWindow, AdapterError> {
    let open = windows_of(pid, deadline)?.1;
    let handles = open
        .iter()
        .map(|popover| popover.handle)
        .collect::<Vec<_>>();
    let handle = choose_popover(&handles, named)?;
    open.into_iter()
        .find(|popover| popover.handle == handle)
        .ok_or_else(no_popover)
}

/// The window a fresh popover capture is rooted at, identified by the same
/// `w-<hwnd>` id `list-surfaces` reports, so the refs it allocates resolve
/// against the popover that was read. The bounds stay unset: the popover is
/// not a census window, and a window-relative bounds space would change what
/// every ref in it means.
pub(crate) fn popover_window(
    app: &AppInfo,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    let popover = resolve_popover(app.pid, None, deadline)?;
    Ok(popover_info(
        app.name.clone(),
        app.pid,
        app.process_instance.clone(),
        popover,
    ))
}

/// The popover window a caller named: a stored ref's recorded id, or the
/// `--window-id` of a capture. The id must still name an open popover of that
/// exact process instance: a closed popover, or one owned by a newer process
/// that reused the pid, is refused instead of resolved.
pub(crate) fn recorded_popover_window(
    process: &ProcessIdentity,
    window_id: &str,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    if !crate::system::process_identity::matches_instance(process.pid, &process.instance)? {
        return Err(popover_closed(window_id, &[]));
    }
    let popover = resolve_popover(process.pid, Some(window_id), deadline)?;
    Ok(popover_info(
        crate::system::process_identity::process_image_name(process.pid).unwrap_or_default(),
        process.pid,
        Some(process.instance.clone()),
        popover,
    ))
}

fn popover_info(
    app: String,
    pid: ProcessId,
    process_instance: Option<String>,
    popover: EnumeratedWindow,
) -> WindowInfo {
    WindowInfo {
        id: handle_id(popover.handle),
        title: live_window_title(popover.handle).unwrap_or_default(),
        app,
        pid,
        process_instance,
        bounds: None,
        state: Default::default(),
    }
}

/// The popover handle a caller means, from the process's open popover handles.
fn choose_popover(
    open: &[WindowHandle],
    named: Option<&str>,
) -> Result<WindowHandle, AdapterError> {
    match named {
        Some(id) => open
            .iter()
            .copied()
            .find(|handle| handle_id(*handle) == id)
            .ok_or_else(|| popover_closed(id, open)),
        None => match open {
            [] => Err(no_popover()),
            [only] => Ok(*only),
            [_, _, ..] => Err(ambiguous_popover(open)),
        },
    }
}

/// The id every window-backed surface on this platform carries, and the id
/// `list-surfaces` reports for the same window.
pub(super) fn handle_id(handle: WindowHandle) -> String {
    format!("w-{}", handle as usize)
}

fn no_popover() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "No popover is open in this application",
    )
}

fn popover_closed(named: &str, open: &[WindowHandle]) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window is not an open popover of this application",
    )
    .with_details(serde_json::json!({
        "kind": "popover_closed",
        "window_id": named,
        "candidates": open.iter().map(|handle| handle_id(*handle)).collect::<Vec<_>>(),
    }))
    .with_suggestion(
        "Run 'list-surfaces' to see the open popover ids, then target the one you mean.",
    )
}

fn ambiguous_popover(open: &[WindowHandle]) -> AdapterError {
    let candidates = open
        .iter()
        .map(|handle| handle_id(*handle))
        .collect::<Vec<_>>();
    AdapterError::new(
        ErrorCode::AmbiguousTarget,
        format!(
            "More than one popover is open in this application: {}",
            candidates.join(", ")
        ),
    )
    .with_details(serde_json::json!({
        "kind": "popover_ambiguous",
        "candidates": candidates,
    }))
    .with_suggestion(
        "Run 'list-surfaces' to see the open popover ids, then read one with 'snapshot --surface popover --app <app> --window-id <id>', or close the popovers you do not need.",
    )
}

#[cfg(test)]
#[path = "popover_choice_tests.rs"]
mod tests;
