//! Resolves a saved ref inside the scope chain it was captured under: counts the
//! nodes of one observation that match a full predicate below one scope, and
//! lands the chain and the element one verified step at a time, each step
//! starting from the live element the step before it verified.

use std::sync::Arc;

use crate::{
    AdapterError, Deadline, IdentityMatch, NativeHandle, PlatformAdapter, RefEntry,
    RefEntryIdentity,
    locator::live_locator::{ObservationRoot, ObservedTree, materialize_ref_entry},
    refs::ref_full_predicate::FullPredicate,
    refs::ref_live_predicate::LivePredicate,
    refs::resolve_errors::{absent, ambiguous_live_match, identity_unknown_error},
    refs::resolve_saved_ref::confirm_landed,
};

/// Why a predicate did not name exactly one node in its scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Miss {
    Absent,
    Ambiguous,
    Unknown,
}

impl Miss {
    pub(crate) fn into_error(self, entry: &RefEntry) -> AdapterError {
        match self {
            Miss::Absent => absent(entry),
            Miss::Ambiguous => ambiguous_live_match(entry),
            Miss::Unknown => identity_unknown_error(entry),
        }
    }
}

/// The one node below `scope` that matches `predicate`, or the whole window
/// when `scope` is none. A second match is `Ambiguous` even when another node
/// could not be read; otherwise an unreadable node or an incomplete observation
/// is `Unknown`, because it could hide a second match.
pub(crate) fn match_in_scope(
    observed: &ObservedTree,
    scope: Option<usize>,
    predicate: &FullPredicate<'_>,
) -> Result<usize, Miss> {
    let mut found = None;
    let mut incomplete = !observed.is_exhaustive();
    for index in members(observed, scope) {
        let live = LivePredicate::observed(&observed.nodes[index].evidence);
        match predicate.compare(&live) {
            IdentityMatch::Match if found.is_some() => return Err(Miss::Ambiguous),
            IdentityMatch::Match => found = Some(index),
            IdentityMatch::Unknown => incomplete = true,
            IdentityMatch::NoMatch => {}
        }
    }
    match found {
        _ if incomplete => Err(Miss::Unknown),
        Some(index) => Ok(index),
        None => Err(Miss::Absent),
    }
}

/// The nodes a recorded chain names in this observation, outermost first: the
/// outermost is unique in the window and each next one is unique below the
/// previous.
pub(crate) fn chain_nodes(
    observed: &ObservedTree,
    chain: &[Arc<RefEntryIdentity>],
) -> Result<Vec<usize>, Miss> {
    let mut nodes = Vec::with_capacity(chain.len());
    for identity in chain.iter().rev() {
        let scope = nodes.last().copied();
        let predicate = FullPredicate::of_identity(identity);
        nodes.push(match_in_scope(observed, scope, &predicate)?);
    }
    Ok(nodes)
}

pub(crate) fn members(observed: &ObservedTree, scope: Option<usize>) -> Vec<usize> {
    let Some(scope) = scope else {
        return (0..observed.nodes.len()).collect();
    };
    let mut found = Vec::new();
    let mut pending: Vec<usize> = children_of(observed, scope);
    while let Some(index) = pending.pop() {
        found.push(index);
        pending.extend(children_of(observed, index));
    }
    found
}

fn children_of(observed: &ObservedTree, index: usize) -> Vec<usize> {
    observed
        .nodes
        .get(index)
        .map(|node| node.children.iter().map(|child| *child as usize).collect())
        .unwrap_or_default()
}

/// Lands the element `found` in `observed` that `entry` names. Each chain
/// ancestor is landed by its path below the previous verified ancestor and
/// confirmed against its recorded identity on the live handle; the element is
/// then landed below the last verified ancestor and confirmed against the
/// entry. The element is a live descendant of the live ancestors that were
/// proven, whatever the tree did since the observation, and a step that does
/// not confirm ends the resolution. An entry with no chain lands from the
/// window root.
pub(crate) fn land_in_scope(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    observed: &ObservedTree,
    found: usize,
    deadline: Deadline,
    lease: Option<&crate::InteractionLease>,
) -> Result<NativeHandle, AdapterError> {
    let ancestors =
        chain_nodes(observed, &entry.scope.chain).map_err(|miss| miss.into_error(entry))?;
    let steps = ancestors
        .into_iter()
        .zip(entry.scope.chain.iter().rev().map(Arc::as_ref))
        .chain([(found, &entry.identity)]);
    let mut base: Option<NativeHandle> = None;
    let mut consumed: &[usize] = &[];
    for (index, identity) in steps {
        let node = observed.nodes.get(index).ok_or_else(|| {
            AdapterError::internal("saved ref match is missing from the observation")
        })?;
        let below = node.path.strip_prefix(consumed).ok_or_else(|| {
            AdapterError::internal("a scope ancestor is not on the element's path")
        })?;
        let mut anchor = materialize_ref_entry(
            node,
            &observed.source,
            &crate::locator::LocatorQuery::default(),
        );
        anchor.scope.path = below.into();
        anchor.identity = RefEntryIdentity {
            unique_at_capture: false,
            ..identity.clone()
        };
        let handle = adapter.resolve_locator_anchor(&anchor, base.as_ref(), deadline)?;
        let root = ObservationRoot::Element {
            handle: &handle,
            entry: &anchor,
            root_ref: None,
        };
        let confirmed = if index == found { entry } else { &anchor };
        confirm_landed(adapter, confirmed, root, deadline, lease)?;
        base = Some(handle);
        consumed = &node.path;
    }
    base.ok_or_else(|| AdapterError::internal("a saved ref landed no element"))
}
