#[cfg(target_os = "macos")]
use agent_desktop_core::{AdapterError, Deadline, RefEntry, SnapshotSurface};

#[cfg(target_os = "macos")]
use crate::tree::AXElement;

#[cfg(target_os = "macos")]
const OWNER_HOPS: usize = 8;

/// Whether a window-surface ref's element still belongs to one of the
/// application's open windows. A closed window keeps answering for its orphaned
/// children, so role and identifier reads alone cannot tell the element is gone.
/// Surfaces that no window owns have nothing to check.
#[cfg(target_os = "macos")]
pub(super) fn owning_window_is_live(
    entry: &RefEntry,
    element: &AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if entry.source.source_surface != SnapshotSurface::Window
        || entry.source.source_window_id.is_none()
    {
        return Ok(true);
    }
    let pid = crate::system::process::process_identity::to_pid_t(entry.process.pid)?;
    let application = crate::tree::properties::element::element_for_pid(pid);
    let Some(container) = read_or_dead(element, "AXWindow", deadline)? else {
        return Ok(false);
    };
    let windows = super::resolve_ax_read::read_array(&application, "AXWindows", deadline)?
        .unwrap_or_default();
    let is_open = |candidate: &AXElement| {
        windows
            .iter()
            .any(|window| crate::tree::properties::capabilities::same_element(window, candidate))
    };
    let mut current = container;
    for _ in 0..OWNER_HOPS {
        if is_open(&current) {
            return Ok(true);
        }
        match read_or_dead(&current, "AXParent", deadline)? {
            Some(parent) => current = parent,
            None => return Ok(false),
        }
    }
    Ok(false)
}

#[cfg(target_os = "macos")]
fn read_or_dead(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    match crate::tree::surface_read::element(element, attribute, deadline) {
        Err(error)
            if error
                .details
                .as_ref()
                .and_then(|details| details["ax_error"].as_i64())
                == Some(i64::from(accessibility_sys::kAXErrorInvalidUIElement)) =>
        {
            Ok(None)
        }
        other => other,
    }
}
