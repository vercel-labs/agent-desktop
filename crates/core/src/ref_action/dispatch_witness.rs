//! What a caller can observe of one delivery, from inside its interaction lease.
//!
//! A witness is told about a dispatch twice: before the action is sent, and once
//! it has settled. Both calls happen while the lease the dispatcher took is
//! still held and before its deadline expires, so a witness can read the desktop
//! in the same window the action itself had, and its reads cannot interleave
//! with another caller's delivery.
//!
//! A witness never decides whether the action succeeded. It observes, and the
//! dispatcher reports exactly what it would have reported without one.

use crate::{ActionResult, AdapterError, Deadline, PlatformAdapter, RefEntry};

/// How one dispatch ended.
pub(crate) enum DispatchOutcome<'a> {
    Delivered(&'a ActionResult),
    Failed(&'a AdapterError),
}

/// One caller's view of the deliveries it is about to make.
pub(crate) trait DispatchWitness {
    /// Called immediately before the action is delivered.
    fn started(
        &self,
        ref_id: &str,
        entry: &RefEntry,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    );

    /// Called once the dispatch has settled, still inside its lease.
    fn settled(
        &self,
        ref_id: &str,
        entry: &RefEntry,
        outcome: DispatchOutcome<'_>,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    );
}
