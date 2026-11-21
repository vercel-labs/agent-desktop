use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::{
    ElementIdentifier, IdentityMatch, RefEntry, RefEntryIdentity, ScopeChain,
    locator::LocatorQuery,
    locator::live_locator::{
        LocatorEvidence, LocatorField, ObservationSource, ObservedTree, materialize_ref_entry,
    },
    refs::ref_full_predicate::{DESCRIPTION, FullPredicate, NAME, NATIVE_ID},
    refs::ref_identity::stable_name,
    refs::ref_live_predicate::LivePredicate,
    refs::ref_scope_resolve::members,
    refs::scope_chain::{MAX_SCOPE_CHAIN_DEPTH, ScopeInterner},
    tree::accname::non_blank,
};

const ROLE: u8 = DESCRIPTION << 1;
type Bucket<'a> = HashMap<LiveKey<'a>, usize>;
const MASKS: usize = (NATIVE_ID | NAME | DESCRIPTION) as usize + 1;

/// One whole-window observation, counted once into the field subsets its
/// entries record, so every tokenless entry published from it is proven by the
/// rule `proves_node` applies rather than by a fresh scan per entry.
///
/// The count is the comparison resolution runs: `FullPredicate::of_entry` of the
/// entry being published, measured against the evidence every node reported, so
/// an identifier chosen by a query or a field that went unread counts the way it
/// will count later.
pub(crate) struct CaptureProof<'a> {
    observed: &'a ObservedTree,
    start: Option<usize>,
    buckets: [OnceLock<HashMap<LiveKey<'a>, usize>>; MASKS],
    unread: Vec<(u8, LivePredicate<'a>)>,
    whole_window: bool,
    parents: OnceLock<Vec<Option<usize>>>,
    scoped: Mutex<ScopeInterner>,
    scope_buckets: Mutex<HashMap<(usize, u8), Arc<Bucket<'a>>>>,
}

impl<'a> CaptureProof<'a> {
    pub(crate) fn new(observed: &'a ObservedTree) -> Self {
        let whole_window = matches!(
            observed.source,
            ObservationSource::Window { .. } | ObservationSource::Application { .. }
        ) && observed.is_exhaustive();
        Self {
            observed,
            whole_window,
            start: observed.roots.first().map(|root| *root as usize),
            buckets: std::array::from_fn(|_| OnceLock::new()),
            parents: OnceLock::new(),
            scoped: Mutex::new(ScopeInterner::default()),
            scope_buckets: Mutex::new(HashMap::new()),
            unread: observed
                .nodes
                .iter()
                .filter_map(|node| {
                    let bits = unread_bits(&node.evidence);
                    (bits != 0).then(|| (bits, LivePredicate::observed(&node.evidence)))
                })
                .collect(),
        }
    }

    /// Starts allocation paths at the node whose observed path is `prefix`, as a
    /// drill does: the prefix is that node's own path, and the rest of an
    /// allocation path counts positions in the projected tree below it.
    pub(crate) fn rooted_at(mut self, prefix: &[usize]) -> Self {
        self.start = self
            .observed
            .nodes
            .iter()
            .position(|node| node.path.as_slice() == prefix);
        self
    }

    /// Whether `entry`, allocated at `path`, is proven. The path is read the way
    /// allocation wrote it: the start node's own path, then child positions in
    /// the projection, which skips no child the observation kept even when the
    /// native indices have gaps.
    pub(crate) fn proves(&self, entry: &RefEntry, path: &[usize]) -> bool {
        self.index_at(path).is_some_and(|index| {
            let mut candidate = entry.clone();
            self.prove_node(&mut candidate, index);
            candidate.identity.unique_at_capture
        })
    }

    /// Builds `entry`, allocated at `path`, under the capture rule.
    pub(crate) fn prove_at(&self, entry: &mut RefEntry, path: &[usize]) {
        if let Some(index) = self.index_at(path) {
            self.prove_node(entry, index);
        }
    }

    fn index_at(&self, path: &[usize]) -> Option<usize> {
        let mut index = self.start?;
        let prefix = self.observed.nodes.get(index)?.path.as_slice();
        for step in path.strip_prefix(prefix)? {
            index = *self.observed.nodes.get(index)?.children.get(*step)? as usize;
        }
        Some(index)
    }

    fn chain_of(&self, predicate: &FullPredicate<'_>, index: usize) -> Option<ScopeChain> {
        if !self.whole_window {
            return None;
        }
        let mut ancestor = self.parent_of(index);
        while let Some(scope) = ancestor {
            if let Some(outer) = self.identified(scope) {
                if !self.proves_in(Some(scope), predicate, index) {
                    return None;
                }
                return self.chain_for_scope(scope, &outer);
            }
            ancestor = self.parent_of(scope);
        }
        None
    }

    fn chain_for_scope(&self, scope: usize, outer: &ScopeChain) -> Option<ScopeChain> {
        if let Some(known) = self.scoped.lock().ok()?.scope_chains.get(&scope) {
            return known.clone();
        }
        let next_len = outer.len().saturating_add(1);
        if next_len > MAX_SCOPE_CHAIN_DEPTH {
            let _ = self
                .scoped
                .lock()
                .map(|mut state| state.scope_chains.insert(scope, None));
            return None;
        }
        let head = self.identity_of(scope)?;
        let mut items = Vec::with_capacity(next_len);
        items.push(head);
        items.extend(outer.iter().cloned());
        self.scoped.lock().ok()?.intern_scope_chain(scope, items)
    }

    fn identified(&self, index: usize) -> Option<ScopeChain> {
        if let Some(known) = self.scoped.lock().ok()?.identified.get(&index) {
            return known.clone();
        }
        let identity = self.identity_of(index)?;
        let predicate = FullPredicate::of_identity(&identity);
        let chain = if self.proves_in(None, &predicate, index) {
            Some(ScopeChain::default())
        } else {
            self.chain_of(&predicate, index)
        };
        self.scoped
            .lock()
            .ok()?
            .identified
            .insert(index, chain.clone());
        chain
    }

    /// What a chain records of one ancestor: the identity its own ref would
    /// publish, with the name reduced to the stable one a resolution compares,
    /// interned once per node within the aggregate scope memory budget.
    fn identity_of(&self, index: usize) -> Option<Arc<RefEntryIdentity>> {
        if let Some(known) = self.scoped.lock().ok()?.identities.get(&index) {
            return known.clone();
        }
        let node = &self.observed.nodes[index];
        let role = non_blank(node.evidence.role.known().map(String::as_str))?;
        let own =
            materialize_ref_entry(node, &self.observed.source, &LocatorQuery::default()).identity;
        let built = RefEntryIdentity {
            name: stable_name(role, own.name.as_deref(), own.value.as_deref()).map(str::to_string),
            retained_object: None,
            value: None,
            ..own
        };
        self.scoped.lock().ok()?.intern_identity(index, built)
    }

    #[cfg(test)]
    pub(crate) fn scope_memory_bytes(&self) -> usize {
        self.scoped
            .lock()
            .map(|state| state.used_bytes)
            .unwrap_or(0)
    }

    fn parent_of(&self, index: usize) -> Option<usize> {
        let parents = self.parents.get_or_init(|| {
            let mut parents = vec![None; self.observed.nodes.len()];
            for (parent, node) in self.observed.nodes.iter().enumerate() {
                for child in &node.children {
                    parents[*child as usize] = Some(parent);
                }
            }
            parents
        });
        parents.get(index).copied().flatten()
    }

    /// Whether `entry` was proven unique in the whole window at the node it was
    /// built from: the observation must be a complete whole-window read, exactly
    /// one of its nodes must match the entry's full predicate, no node may leave
    /// a recorded field unread, and that one node must be this one.
    pub(crate) fn proves_node(&self, entry: &RefEntry, index: usize) -> bool {
        self.proves_in(None, &FullPredicate::of_entry(entry), index)
    }

    /// The one capture rule every ref entry is built under: records on `entry`,
    /// built from node `index`, whether it is proven and the scope chain that
    /// proves it when its own proof is not unique in the window.
    pub(crate) fn prove_node(&self, entry: &mut RefEntry, index: usize) {
        entry.identity.unique_at_capture = self.proves_node(entry, index);
        if entry.identity.unique_at_capture {
            return;
        }
        if let Some(chain) = self.chain_of(&FullPredicate::of_entry(entry), index) {
            entry.identity.unique_at_capture = true;
            entry.scope.chain = chain;
        }
    }

    fn proves_in(&self, scope: Option<usize>, predicate: &FullPredicate<'_>, index: usize) -> bool {
        if !self.whole_window {
            return false;
        }
        let fields = predicate.mask();
        let key = key_of(predicate);
        let unique = match scope {
            None => self.buckets[fields as usize]
                .get_or_init(|| count_nodes(self.observed, fields, 0..self.observed.nodes.len()))
                .get(&key)
                .copied(),
            Some(scope) => self.scope_bucket(scope, fields).get(&key).copied(),
        };
        unique == Some(index)
            && !self.unread.iter().any(|(bits, live)| {
                bits & (fields | ROLE) != 0 && predicate.compare(live) == IdentityMatch::Unknown
            })
    }

    fn scope_bucket(&self, scope: usize, fields: u8) -> Arc<Bucket<'a>> {
        let Ok(mut buckets) = self.scope_buckets.lock() else {
            return Arc::default();
        };
        Arc::clone(buckets.entry((scope, fields)).or_insert_with(|| {
            Arc::new(count_nodes(
                self.observed,
                fields,
                members(self.observed, Some(scope)),
            ))
        }))
    }
}

/// The node indices each full predicate of this field subset matched, as a
/// single index per predicate. A predicate two nodes match is stored with the
/// last of them, so it never equals the node an entry was built from.
fn count_nodes(
    observed: &ObservedTree,
    fields: u8,
    members: impl IntoIterator<Item = usize>,
) -> HashMap<LiveKey<'_>, usize> {
    let mut counted: HashMap<LiveKey<'_>, (usize, usize)> = HashMap::new();
    for index in members {
        for key in keys_of(&observed.nodes[index].evidence, fields) {
            let entry = counted.entry(key).or_insert((0, index));
            entry.0 += 1;
            entry.1 = index;
        }
    }
    counted
        .into_iter()
        .filter(|(_, (count, _))| *count == 1)
        .map(|(key, (_, index))| (key, index))
        .collect()
}

fn keys_of<'a>(evidence: &'a LocatorEvidence, fields: u8) -> Vec<LiveKey<'a>> {
    let Some(role) = non_blank(evidence.role.known().map(String::as_str)) else {
        return Vec::new();
    };
    let name = match fields & NAME {
        0 => None,
        _ => match stable_name(
            role,
            evidence.name.known().map(String::as_str),
            evidence.value.known().map(String::as_str),
        ) {
            Some(name) => Some(name),
            None => return Vec::new(),
        },
    };
    let description = match fields & DESCRIPTION {
        0 => None,
        _ => match non_blank(evidence.description.known().map(String::as_str)) {
            Some(description) => Some(description),
            None => return Vec::new(),
        },
    };
    let key = |native_id: Option<&'a ElementIdentifier>| LiveKey {
        role,
        name,
        description,
        native_id,
    };
    if fields & NATIVE_ID == 0 {
        return vec![key(None)];
    }
    evidence
        .identifiers
        .identifiers()
        .iter()
        .map(|native_id| key(Some(native_id)))
        .collect()
}

/// The fields this element could not answer, so a predicate that records one of
/// them reaches `Unknown` here exactly as it will at resolution.
fn unread_bits(evidence: &LocatorEvidence) -> u8 {
    let live = LivePredicate::observed(evidence);
    let mut bits = 0;
    match &live.role {
        LocatorField::Known(role) => {
            if live.stable_name(role).is_unknown() {
                bits |= NAME;
            }
        }
        LocatorField::Unknown => bits |= ROLE,
        LocatorField::Absent => {}
    }
    if live.description.is_unknown() {
        bits |= DESCRIPTION;
    }
    if !live.native_ids_complete {
        bits |= NATIVE_ID;
    }
    bits
}

fn key_of<'b>(predicate: &FullPredicate<'b>) -> LiveKey<'b> {
    LiveKey {
        role: predicate.role,
        name: predicate.name,
        description: predicate.description,
        native_id: predicate.native_id,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct LiveKey<'a> {
    role: &'a str,
    name: Option<&'a str>,
    description: Option<&'a str>,
    native_id: Option<&'a ElementIdentifier>,
}

#[cfg(test)]
#[path = "ref_capture_proof_tests.rs"]
mod tests;
