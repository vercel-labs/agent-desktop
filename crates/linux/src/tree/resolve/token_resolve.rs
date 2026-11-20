use agent_desktop_core::refs::ref_identity::element_token;
use agent_desktop_core::refs::resolve_errors::{element_token_stale, identity_unknown_error};
use agent_desktop_core::{AdapterError, Deadline, RefEntry};

use super::{Source, settle_read_error};
use crate::system::REGISTRY_ROOT_PATH;
use crate::tree::AtspiElement;
use crate::tree::element::element_token::ElementToken;

/// Resolves a token ref by reading its recorded chain again, with no search.
///
/// The bus instance, the window's process, start time and bus owner, every
/// parent-child link from the application root (for a shell surface, from its
/// verified container) down to the element, and the element's identity
/// fingerprint must all still hold: the element read now must reproduce the
/// stored token exactly. A closed window whose
/// objects still answer fails at the application root, a recycled row fails
/// at its parent or at its name, and an identical twin is told apart by its
/// own address.
pub(crate) fn resolve_token<S: Source>(
    source: &mut S,
    entry: &RefEntry,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<AtspiElement, AdapterError> {
    if source.bus_guid()? != token.guid {
        return Err(element_token_stale(entry, "bus", None));
    }
    let frame = source.verify_window(entry)?;
    if frame.bus_name() != token.bus || frame.path() != token.frame() || frame.pid() != token.pid {
        return Err(element_token_stale(entry, "window", None));
    }
    if !source.is_shell_frame(&frame) {
        let root = AtspiElement::new(token.bus.as_str(), REGISTRY_ROOT_PATH, token.pid);
        child_at(source, entry, &root, &frame, deadline).map_err(|error| {
            error.unwrap_or_else(|| element_token_stale(entry, "chain", Some(0)))
        })?;
    }
    let mut current = frame;
    for (depth, path) in token.paths.iter().enumerate().skip(1) {
        let next = AtspiElement::new(token.bus.as_str(), path.as_str(), token.pid);
        current = child_at(source, entry, &current, &next, deadline).map_err(|error| {
            error.unwrap_or_else(|| element_token_stale(entry, "chain", Some(depth)))
        })?;
    }
    let evidence = source
        .evidence(&current)
        .map_err(|error| settle_read_error(source, entry, &current, error))?;
    match evidence.identifiers.element_token() {
        Some(live) if Some(live) == element_token(entry) => Ok(current),
        Some(_) => Err(element_token_stale(entry, "identity", None)),
        None => Err(identity_unknown_error(entry)),
    }
}

/// The child of `parent` at `next`'s address, carrying the ancestry the walk
/// gives it. `Err(None)` is a link that no longer holds.
fn child_at<S: Source>(
    source: &mut S,
    entry: &RefEntry,
    parent: &AtspiElement,
    next: &AtspiElement,
    deadline: Deadline,
) -> Result<AtspiElement, Option<AdapterError>> {
    if deadline.is_expired() {
        return Err(Some(deadline.timeout_error()));
    }
    let level = source
        .level(parent)
        .map_err(|error| Some(settle_read_error(source, entry, parent, error)))?;
    if level.children_unread || (level.children.is_empty() && level.empty_means_unread) {
        return Err(Some(identity_unknown_error(entry)));
    }
    level
        .children
        .into_iter()
        .find(|child| child.bus_name() == next.bus_name() && child.path() == next.path())
        .ok_or(None)
}

#[cfg(test)]
#[path = "token_resolve_tests.rs"]
mod tests;
