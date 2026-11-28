use crate::{
    AdapterError, Deadline, NativeHandle, RefEntry,
    adapter::PlatformAdapter,
    locator::live_locator::ObservationRoot,
    refs::ref_identity::element_token,
    refs::resolve_saved_ref::{confirm_landed, confirm_landed_element, resolve_saved_ref},
};

/// Lands a tokenless saved ref at its recorded path and confirms the landed
/// element against the full predicate, skipping the whole-window count.
///
/// The count is what proves the ref still unique, so this handle may only feed
/// a wait that runs a full [`resolve_saved_ref`] under the interaction lease
/// before acting, or a presentation that never acts, such as placing the cursor
/// overlay. A landing that fails or does not confirm falls back to the full
/// resolution, so a moved, missing or twinned element gets the same verdict
/// either way and only the common case is cheaper.
/// Lands a tokenless saved ref at its recorded path and confirms the landed
/// element against the full predicate, skipping the whole-window count, and says whether the handle comes from a full
/// resolution, which counted the live matches, rather than from a landing.
pub(crate) fn land_saved_ref_counted(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<(NativeHandle, bool), AdapterError> {
    if element_token(entry).is_none()
        && entry.identity.proof.is_unique()
        && entry.scope.chain.is_empty()
        && let Ok(handle) = adapter.resolve_locator_anchor(entry, None, deadline)
        && confirm_landed_element(adapter, entry, &handle, deadline).is_ok()
    {
        return Ok((handle, false));
    }
    resolve_saved_ref(adapter, entry, deadline, None).map(|handle| (handle, true))
}

/// The check an action makes under the interaction lease on a handle it
/// resolved before taking it: the element is still live and still matches the
/// recorded predicate. It reads the one element and does not count the window
/// again, so a slow whole-window read never runs while the lease is held.
pub(crate) fn confirm_resolved_under_lease(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    handle: &NativeHandle,
    deadline: Deadline,
    lease: &crate::InteractionLease,
) -> Result<(), AdapterError> {
    let root = ObservationRoot::Element {
        handle,
        entry,
        root_ref: None,
    };
    confirm_landed(adapter, entry, root, deadline, Some(lease))
}
