use crate::{ActionResult, Deadline, InteractionLease, NativeHandle};

/// A delivered ref action, the element it landed on, and the lease and
/// artifacts the caller still owns: it releases the lease and finishes the
/// artifacts after its own post-action work.
pub(crate) struct RefActionLanding {
    pub(crate) result: ActionResult,
    pub(crate) handle: NativeHandle,
    pub(crate) lease: InteractionLease,
    pub(crate) pre: crate::trace::trace_artifacts::ArtifactOutcome,
    pub(crate) deadline: Deadline,
    pub(crate) lease_started: std::time::Instant,
}
