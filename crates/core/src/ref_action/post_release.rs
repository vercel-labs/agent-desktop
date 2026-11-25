use crate::actions::witnessed_action::PendingWitness;
use crate::{
    ActionRequest, ActionResult, AdapterError, Deadline, InteractionLease, NativeHandle,
    PlatformAdapter, ProcessIdentity,
};

/// The reads a delivered ref action still owes once the interaction lease is
/// released: the effect witness after the dispatch, and the overlays an action
/// that may open a window raised. The lease covers resolving the target and
/// dispatching to it. An application that has just opened a modal answers
/// slowly, and no other agent should wait behind that.
#[derive(Default)]
pub(crate) struct PostRelease {
    pub(crate) surfaces_of: Option<ProcessIdentity>,
    pub(crate) witness: Option<PendingWitness>,
}

impl PostRelease {
    pub(crate) fn settle(
        self,
        adapter: &dyn PlatformAdapter,
        landed_on: &NativeHandle,
        deadline: Deadline,
        result: ActionResult,
    ) -> ActionResult {
        let result = match self.witness {
            Some(witness) => witness.settle(adapter, landed_on, deadline, result),
            None => result,
        };
        match self.surfaces_of {
            Some(process) => {
                result.with_surfaces_read(adapter.surfaces_raised_by(process, landed_on, deadline))
            }
            None => result,
        }
    }
}

/// Delivers the action under the lease and returns the witness read it still
/// owes, to run after the release.
pub(crate) fn dispatch_deferred(
    adapter: &dyn PlatformAdapter,
    handle: &NativeHandle,
    request: ActionRequest,
    lease: &InteractionLease,
) -> (Result<ActionResult, AdapterError>, Option<PendingWitness>) {
    match crate::actions::post_action::execute_verified_deferred(adapter, handle, request, lease) {
        Ok((result, witness)) => (Ok(result), witness),
        Err(error) => (Err(error), None),
    }
}
