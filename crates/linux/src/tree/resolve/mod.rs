use agent_desktop_core::refs::ref_identity::element_token;
use agent_desktop_core::refs::resolve_errors::element_token_required;
use agent_desktop_core::{AdapterError, Deadline, ErrorCode, LocatorEvidence, RefEntry};

use super::AtspiElement;
use super::element::element_token::decode;
use super::resolve::resolve_errors::{
    descendant_owner_gone, is_incomplete, settle_gone, stopped_aware,
};
use super::resolve::token_resolve::resolve_token;
use super::walk::level::Level;

/// What a resolution reads. The bus implements it; a test substitutes an
/// in-memory tree and counts the reads.
pub(crate) trait Source {
    /// The guid of the accessibility bus instance the reads go to.
    fn bus_guid(&mut self) -> Result<String, AdapterError>;
    /// The ref's window frame once its process, start time and bus owner are
    /// proved the stored ones and the frame answers.
    fn verify_window(&mut self, entry: &RefEntry) -> Result<AtspiElement, AdapterError>;
    /// Whether the frame is a shell surface container, which its application
    /// root does not list directly.
    fn is_shell_frame(&mut self, frame: &AtspiElement) -> bool;
    /// The verified root of the ref's surface, carrying its window ancestry.
    fn open_window(&mut self, entry: &RefEntry) -> Result<AtspiElement, AdapterError>;
    /// The children of one node, in the order a walk records them.
    fn level(&mut self, element: &AtspiElement) -> Result<Level<AtspiElement>, AdapterError>;
    /// The evidence of one node, read as the snapshot walk reads it.
    fn evidence(&mut self, element: &AtspiElement) -> Result<LocatorEvidence, AdapterError>;
    /// The listed name of the application when `/proc` shows it stopped.
    fn stopped_app(&mut self, pid: u32) -> Option<String>;
}

pub(super) fn settle_read_error<S: Source>(
    source: &mut S,
    entry: &RefEntry,
    element: &AtspiElement,
    error: AdapterError,
) -> AdapterError {
    if error.code != ErrorCode::AppNotFound
        || element
            .frame()
            .is_none_or(|frame| frame.bus_name() == element.bus_name())
    {
        return error;
    }
    match source.open_window(entry) {
        Ok(_) => descendant_owner_gone(element, error),
        Err(error) => error,
    }
}

/// Resolves a stored ref to the live element it was taken from, or refuses.
/// The ref's element token is read back along its recorded chain; nothing is
/// ever searched for, so a ref without a token is refused. An application
/// that left, a destroyed object and a timeout are told apart at this
/// boundary, and a timeout on a stopped process is `APP_UNRESPONSIVE`.
pub(crate) fn resolve<S: Source>(
    source: &mut S,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<AtspiElement, AdapterError> {
    if let Some(name) = source.stopped_app(entry.process.pid.get()) {
        return Err(AdapterError::app_unresponsive(&name));
    }
    let Some(token) = element_token(entry).and_then(decode) else {
        return Err(element_token_required(entry));
    };
    let outcome = resolve_token(source, entry, &token, deadline);
    settle_outcome(source, entry, outcome)
}

/// Maps the end of one attempt to the answer the caller gets: an application
/// that left and a destroyed object are settled refusals, and a timeout on a
/// stopped process is `APP_UNRESPONSIVE`.
pub(super) fn settle_outcome<S: Source>(
    source: &mut S,
    entry: &RefEntry,
    outcome: Result<AtspiElement, AdapterError>,
) -> Result<AtspiElement, AdapterError> {
    outcome.map_err(|error| {
        let error = settle_gone(error, entry.process.pid.get());
        if error.code == ErrorCode::Timeout || is_incomplete(&error) {
            let stopped = source.stopped_app(entry.process.pid.get());
            stopped_aware(error, stopped.as_deref())
        } else {
            error
        }
    })
}

pub(super) mod landing;
pub(super) mod path_descent;
pub(super) mod resolve_anchor;
pub(super) mod resolve_errors;
#[cfg(test)]
pub(super) mod resolve_fake;
pub(super) mod resolve_gate;
pub(super) mod resolve_source;
pub(super) mod token_resolve;
