use crate::actions::witnessed_action::PendingWitness;
use crate::system::front_pending::FrontPending;
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
    pub(crate) front: Option<FrontPending>,
}

impl PostRelease {
    pub(crate) fn settle(
        self,
        adapter: &dyn PlatformAdapter,
        landed_on: &NativeHandle,
        deadline: Deadline,
        result: ActionResult,
    ) -> ActionResult {
        let surfaces_of = self.surfaces_of;
        let witness = self.witness;
        let work = move || {
            let result = match witness {
                Some(witness) => witness.settle(adapter, landed_on, deadline, result),
                None => result,
            };
            match surfaces_of {
                Some(process) => result
                    .with_surfaces_read(adapter.surfaces_raised_by(process, landed_on, deadline)),
                None => result,
            }
        };
        let Some(front) = self.front else {
            return work();
        };
        let (mut result, resolution) = front.run_around(adapter, work);
        resolution.outcome.note_in(&mut result.details);
        if let Some(reason) = resolution.pending_reason {
            crate::FrontOutcome::note_watch_in(reason, &mut result.details);
        }
        result
    }
}

/// Delivers the action under the lease and returns the witness read it still
/// owes, to run after the release.
pub(crate) fn dispatch_deferred(
    adapter: &dyn PlatformAdapter,
    handle: &NativeHandle,
    request: ActionRequest,
    lease: &InteractionLease,
) -> (
    Result<ActionResult, AdapterError>,
    Option<PendingWitness>,
    Option<FrontPending>,
) {
    match crate::actions::post_action::execute_verified_deferred(adapter, handle, request, lease) {
        Ok((result, witness, front)) => (Ok(result), witness, front),
        Err(error) => (Err(error), None, None),
    }
}
