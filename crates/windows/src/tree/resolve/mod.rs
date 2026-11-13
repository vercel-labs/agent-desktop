use agent_desktop_core::{AdapterError, Deadline, NativeHandle, RefEntry};

#[cfg(target_os = "windows")]
use super::element::UIAElement;
#[cfg(target_os = "windows")]
use super::resolve::resolve_match::owning_process_exited_error;
#[cfg(target_os = "windows")]
use super::resolve::resolve_match::stale_ref_error;

/// Wraps the resolved element with the ref's verified process identity, so the
/// live-read path can corroborate that the provider is still the one
/// resolution verified. The token-less case cannot reach here:
/// `resolve_window_root` fails closed on it first.
#[cfg(target_os = "windows")]
pub(crate) fn into_verified_handle(
    element: super::element::UIAElement,
    entry: &RefEntry,
) -> NativeHandle {
    element
        .with_verified_process(
            entry.process.pid.get(),
            entry.process.process_instance.clone().unwrap_or_default(),
        )
        .into_native_handle()
}

/// Reaches the stored window's root element from the ref's source window id.
///
/// The fail-closed window gate (the A7-3 wrong-target shape exists to
/// prevent): a stored ref must not search the tree of a different process that
/// has since taken over the HWND. The stored window the ref names is
/// reconstructed and put through the same stored-evidence rule
/// (`WindowIdentityEvidence::verify_stored`) the listing path uses, so the
/// live owner of the handle, not merely the liveness of the stored pid,
/// decides whether the window is still the ref's window. The macOS resolver
/// verifies its window record before searching in the same position. A
/// token-less ref (elevated process whose token could not be read) yields no
/// evidence and fails closed here rather than searching an unverified window.
///
/// The rule's own error is `WINDOW_NOT_FOUND`; the resolver reports its
/// failures as `STALE_REF`, so the verdict is adopted and the error contract
/// of this path is unchanged.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_window_root(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<UIAElement, AdapterError> {
    let window_id = entry
        .source
        .source_window_id
        .as_deref()
        .ok_or_else(|| stale_ref_error(entry))?;
    let window = agent_desktop_core::WindowInfo {
        id: window_id.to_string(),
        title: entry.source.source_window_title.clone().unwrap_or_default(),
        app: entry.source.source_app.clone().unwrap_or_default(),
        pid: entry.process.pid,
        process_instance: entry.process.process_instance.clone(),
        bounds: None,
        state: Default::default(),
    };
    crate::system::window::window_identity::WindowIdentityEvidence::from_info(
        crate::system::window::window_ops::parse_handle(&window.id),
        &window,
    )
    .ok_or_else(|| stale_ref_error(entry))?
    .verify_stored()
    .map_err(|_| verify_stored_failure_error(entry, deadline))?;

    crate::tree::surfaces::surface_root(
        agent_desktop_core::ObservationRoot::Window(&window),
        entry.source.source_surface,
        deadline,
    )
}

/// Classifies a `verify_stored()` failure by the owning process's own
/// liveness, not merely the handle's: a window recreated mid-redraw by a
/// still-running process is retryable, an exited owner is terminal. Without
/// this split a dead owner's every re-attempt re-fails identically and the
/// retry loop burns the whole deadline before reporting a bare `TIMEOUT`
/// with no diagnosis. Answers only when the ref carries a real
/// process-instance token; a token-less ref keeps today's retryable
/// behaviour, since liveness cannot be checked without one.
#[cfg(target_os = "windows")]
fn verify_stored_failure_error(entry: &RefEntry, deadline: Deadline) -> AdapterError {
    let Some(instance) = entry
        .process
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
    else {
        return stale_ref_error(entry);
    };
    let identity = agent_desktop_core::ProcessIdentity::new(entry.process.pid, instance);
    match crate::system::process_state::process_state_impl(identity, deadline) {
        Ok(
            agent_desktop_core::system::process_state::ProcessState::Exited { .. }
            | agent_desktop_core::system::process_state::ProcessState::Crashed { .. },
        ) => owning_process_exited_error(entry),
        _ => stale_ref_error(entry),
    }
}

pub(crate) mod resolve_anchor;
pub(crate) mod resolve_match;
pub(crate) mod resolve_search;
