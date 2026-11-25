use std::collections::HashMap;
use std::sync::OnceLock;

use crate::{
    ElementIdentifier, IdentityMatch, IdentityProof, RefEntry,
    locator::live_locator::{LocatorEvidence, LocatorField, ObservationSource, ObservedTree},
    refs::ref_full_predicate::{DESCRIPTION, FullPredicate, NAME, NATIVE_ID},
    refs::ref_identity::stable_name,
    refs::ref_live_predicate::LivePredicate,
    tree::accname::non_blank,
};

const ROLE: u8 = DESCRIPTION << 1;
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
    buckets: [OnceLock<HashMap<LiveKey<'a>, Census>>; MASKS],
    unread: Vec<(u8, LivePredicate<'a>)>,
    whole_window: bool,
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
    pub(crate) fn proof_at(&self, entry: &RefEntry, path: &[usize]) -> IdentityProof {
        let Some(mut index) = self.start else {
            return IdentityProof::Unproven;
        };
        let Some(prefix) = self
            .observed
            .nodes
            .get(index)
            .map(|node| node.path.as_slice())
        else {
            return IdentityProof::Unproven;
        };
        let Some(steps) = path.strip_prefix(prefix) else {
            return IdentityProof::Unproven;
        };
        for step in steps {
            match self
                .observed
                .nodes
                .get(index)
                .and_then(|node| node.children.get(*step))
            {
                Some(child) => index = *child as usize,
                None => return IdentityProof::Unproven,
            }
        }
        self.proof_of_node(entry, index)
    }

    pub(crate) fn proves(&self, entry: &RefEntry, path: &[usize]) -> bool {
        self.proof_at(entry, path).is_unique()
    }

    pub(crate) fn proves_node(&self, entry: &RefEntry, index: usize) -> bool {
        self.proof_of_node(entry, index).is_unique()
    }

    /// What the observation establishes about `entry` at the node it was built
    /// from. `Unique` needs a complete whole-window read in which exactly one
    /// node matches the entry's full predicate, that node is this one, and no
    /// node leaves a recorded field unread. Two or more matching nodes are
    /// `Twins` whatever else went unread, because both matches were read. Any
    /// other outcome leaves the entry `Unproven`.
    pub(crate) fn proof_of_node(&self, entry: &RefEntry, index: usize) -> IdentityProof {
        if !self.whole_window {
            return IdentityProof::Unproven;
        }
        let predicate = FullPredicate::of_entry(entry);
        let fields = predicate.mask();
        let census = self.buckets[fields as usize]
            .get_or_init(|| count_nodes(self.observed, fields))
            .get(&key_of(&predicate))
            .copied();
        match census {
            Some(Census {
                count: 1,
                index: only,
            }) if only == index => {}
            Some(Census { count, .. }) if count > 1 => return IdentityProof::Twins,
            _ => return IdentityProof::Unproven,
        }
        let unread = self.unread.iter().any(|(bits, live)| {
            bits & (fields | ROLE) != 0 && predicate.compare(live) == IdentityMatch::Unknown
        });
        if unread {
            IdentityProof::Unproven
        } else {
            IdentityProof::Unique
        }
    }
}

/// How many nodes matched one full predicate, and the last of them.
#[derive(Clone, Copy)]
struct Census {
    count: usize,
    index: usize,
}

/// The census each full predicate of this field subset received. A predicate
/// two nodes match is recorded with both, so it is `Twins` for each of them.
fn count_nodes(observed: &ObservedTree, fields: u8) -> HashMap<LiveKey<'_>, Census> {
    let mut counted: HashMap<LiveKey<'_>, Census> = HashMap::new();
    for (index, node) in observed.nodes.iter().enumerate() {
        for key in keys_of(&node.evidence, fields) {
            let census = counted.entry(key).or_insert(Census { count: 0, index });
            census.count += 1;
            census.index = index;
        }
    }
    counted
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
