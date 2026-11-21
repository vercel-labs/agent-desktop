use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::RefEntryIdentity;

pub(crate) const MAX_SCOPE_MEMORY_BYTES: usize = crate::refs::MAX_REFMAP_BYTES as usize;
pub(crate) const MAX_SCOPE_CHAIN_DEPTH: usize = 256;

/// Shared immutable sequence of ancestor identities that scope a non-unique
/// ref, nearest first. Empty chains allocate nothing, and cloning a populated
/// chain increments a single reference count rather than copying ancestor
/// strings.
#[derive(Debug, Clone, Default, Eq)]
pub struct ScopeChain(Option<Arc<[Arc<RefEntryIdentity>]>>);

impl ScopeChain {
    /// Builds a shared chain from already-interned ancestor identities.
    pub fn from_shared(items: Vec<Arc<RefEntryIdentity>>) -> Self {
        if items.is_empty() {
            Self(None)
        } else {
            Self(Some(Arc::from(items)))
        }
    }

    /// Builds a shared chain by wrapping owned ancestor identities once.
    pub fn from_owned(items: Vec<RefEntryIdentity>) -> Self {
        Self::from_shared(items.into_iter().map(Arc::new).collect())
    }

    /// Returns the ancestor identities as a slice.
    pub fn as_slice(&self) -> &[Arc<RefEntryIdentity>] {
        self.0.as_deref().unwrap_or(&[])
    }

    /// Returns `true` when no ancestor scopes this ref.
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    /// Returns the number of ancestors in this scope chain.
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Clears the scope chain without leaving a heap allocation behind.
    pub fn clear(&mut self) {
        self.0 = None;
    }

    /// Reports whether two chains share the same underlying allocation.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    pub(crate) fn ptr_key(&self) -> Option<(usize, usize)> {
        let arc = self.0.as_ref()?;
        Some((arc.as_ptr() as usize, arc.len()))
    }
}

impl Deref for ScopeChain {
    type Target = [Arc<RefEntryIdentity>];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl PartialEq for ScopeChain {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other) || self.as_slice() == other.as_slice()
    }
}

impl Hash for ScopeChain {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl From<Vec<RefEntryIdentity>> for ScopeChain {
    fn from(items: Vec<RefEntryIdentity>) -> Self {
        Self::from_owned(items)
    }
}

impl Serialize for ScopeChain {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let items = self.as_slice();
        let mut seq = serializer.serialize_seq(Some(items.len()))?;
        for item in items {
            seq.serialize_element(item.as_ref())?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for ScopeChain {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let items = Vec::<RefEntryIdentity>::deserialize(deserializer)?;
        if items.len() > MAX_SCOPE_CHAIN_DEPTH {
            return Err(serde::de::Error::custom(
                "RefEntry scope chain exceeds maximum depth",
            ));
        }
        Ok(Self::from_owned(items))
    }
}

pub(crate) fn identity_memory_bytes(identity: &RefEntryIdentity) -> usize {
    std::mem::size_of::<RefEntryIdentity>()
        .saturating_add(identity.role.len())
        .saturating_add(identity.retained_object.as_ref().map_or(0, String::len))
        .saturating_add(identity.name.as_ref().map_or(0, String::len))
        .saturating_add(identity.value.as_ref().map_or(0, String::len))
        .saturating_add(identity.description.as_ref().map_or(0, String::len))
        .saturating_add(identity.native_id.as_ref().map_or(0, |id| {
            std::mem::size_of_val(id).saturating_add(id.value.len())
        }))
}

pub(crate) fn chain_slots_bytes(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let header = std::mem::size_of::<usize>().saturating_mul(2);
    header.saturating_add(len.saturating_mul(std::mem::size_of::<Arc<RefEntryIdentity>>()))
}

#[derive(Default)]
pub(crate) struct ScopeInterner {
    unique_identities: HashSet<Arc<RefEntryIdentity>>,
    pub(crate) identities: HashMap<usize, Option<Arc<RefEntryIdentity>>>,
    pub(crate) identified: HashMap<usize, Option<ScopeChain>>,
    pub(crate) scope_chains: HashMap<usize, Option<ScopeChain>>,
    pub(crate) used_bytes: usize,
}

impl ScopeInterner {
    pub(crate) fn try_charge(&mut self, bytes: usize) -> bool {
        let Some(next) = self.used_bytes.checked_add(bytes) else {
            return false;
        };
        if next > MAX_SCOPE_MEMORY_BYTES {
            return false;
        }
        self.used_bytes = next;
        true
    }

    pub(crate) fn intern_identity(
        &mut self,
        index: usize,
        built: RefEntryIdentity,
    ) -> Option<Arc<RefEntryIdentity>> {
        if let Some(known) = self.identities.get(&index) {
            return known.clone();
        }
        let shared = Arc::new(built);
        if let Some(existing) = self.unique_identities.get(&shared) {
            let reused = Arc::clone(existing);
            self.identities.insert(index, Some(Arc::clone(&reused)));
            return Some(reused);
        }
        if !self.try_charge(identity_memory_bytes(&shared)) {
            self.identities.insert(index, None);
            return None;
        }
        self.unique_identities.insert(Arc::clone(&shared));
        self.identities.insert(index, Some(Arc::clone(&shared)));
        Some(shared)
    }

    pub(crate) fn intern_scope_chain(
        &mut self,
        scope: usize,
        items: Vec<Arc<RefEntryIdentity>>,
    ) -> Option<ScopeChain> {
        if let Some(known) = self.scope_chains.get(&scope) {
            return known.clone();
        }
        if !self.try_charge(chain_slots_bytes(items.len())) {
            self.scope_chains.insert(scope, None);
            return None;
        }
        let shared = ScopeChain::from_shared(items);
        self.scope_chains.insert(scope, Some(shared.clone()));
        Some(shared)
    }
}

#[cfg(test)]
#[path = "scope_chain_tests.rs"]
mod tests;
