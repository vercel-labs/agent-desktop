use crate::ref_action::{
    mark_pre_dispatch_resolution_failure, ref_action_context::RefActionContext,
    ref_action_wait_support::trace_resolve_error,
};
use crate::refs::ref_identity::element_token;
use crate::refs::resolve_saved_ref::{resolve_saved_ref, saved_ref_landing};
use crate::{AdapterError, InteractionLease, NativeHandle};

/// The handle a dispatch acts on, resolved under the interaction lease. A
/// handle the wait proved before the lease is only confirmed: the element is
/// read once more and must still match the recorded identity. Without one, the
/// ref is resolved in full, which counts the live matches under the lease. A
/// ref that carries an element token is always resolved through the adapter,
/// which owns its re-identification, so it never takes the confirmation read.
/// A tokenless ref with a non-empty ancestor scope chain is also resolved in
/// full under the lease, because confirming only the leaf would miss a recycled
/// ancestor row whose child kept the same identity and handle.
pub(super) fn resolve_for_dispatch(
    target: &RefActionContext<'_>,
    resolved: Option<NativeHandle>,
    lease: &InteractionLease,
) -> Result<NativeHandle, AdapterError> {
    match resolved
        .filter(|_| element_token(target.entry).is_none() && target.entry.scope.chain.is_empty())
    {
        Some(handle) => saved_ref_landing::confirm_resolved_under_lease(
            target.adapter,
            target.entry,
            &handle,
            target.deadline,
            lease,
        )
        .map(|()| handle),
        None => resolve_saved_ref(target.adapter, target.entry, target.deadline, Some(lease)),
    }
    .map_err(mark_pre_dispatch_resolution_failure)
    .inspect_err(|error| trace_resolve_error(target.context, target.ref_id, error))
}

#[cfg(test)]
#[path = "ref_action_resolve_tests.rs"]
mod tests;
