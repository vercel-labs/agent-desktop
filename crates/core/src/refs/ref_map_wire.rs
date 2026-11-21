use crate::refs::RefMap;
use crate::refs::scope_chain::{
    MAX_SCOPE_CHAIN_DEPTH, MAX_SCOPE_MEMORY_BYTES, chain_slots_bytes, identity_memory_bytes,
};
use crate::{RefEntry, RefEntryIdentity, ScopeChain};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

const MISSING_CHAIN: &str = "RefMap names a scope chain it does not hold";
const BUDGET_EXCEEDED: &str = "RefMap scope chains exceed the aggregate memory budget";
const DEPTH_EXCEEDED: &str = "RefMap scope chain exceeds maximum depth";

struct WireScopeBudget {
    identities: HashSet<Arc<RefEntryIdentity>>,
    used_bytes: usize,
}

impl WireScopeBudget {
    fn new() -> Self {
        Self {
            identities: HashSet::new(),
            used_bytes: 0,
        }
    }

    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        let next = self
            .used_bytes
            .checked_add(bytes)
            .ok_or_else(|| BUDGET_EXCEEDED.to_owned())?;
        if next > MAX_SCOPE_MEMORY_BYTES {
            return Err(BUDGET_EXCEEDED.to_owned());
        }
        self.used_bytes = next;
        Ok(())
    }

    fn intern_identity(
        &mut self,
        identity: Arc<RefEntryIdentity>,
    ) -> Result<Arc<RefEntryIdentity>, String> {
        if let Some(existing) = self.identities.get(&identity) {
            return Ok(Arc::clone(existing));
        }
        self.charge(identity_memory_bytes(&identity))?;
        self.identities.insert(Arc::clone(&identity));
        Ok(identity)
    }

    fn intern_chain(&mut self, chain: &ScopeChain) -> Result<ScopeChain, String> {
        if chain.is_empty() {
            return Ok(ScopeChain::default());
        }
        if chain.len() > MAX_SCOPE_CHAIN_DEPTH {
            return Err(DEPTH_EXCEEDED.to_owned());
        }
        let mut items = Vec::with_capacity(chain.len());
        for identity in chain.iter() {
            items.push(self.intern_identity(Arc::clone(identity))?);
        }
        self.charge(chain_slots_bytes(items.len()))?;
        Ok(ScopeChain::from_shared(items))
    }
}

/// The stored form of a `RefMap`. Each distinct scope chain is written once in
/// `chains`, and `chain_of` maps a ref id to its chain's index. Refmaps saved
/// before this form carry their chains inline on the entries and still load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RefMapWire {
    inner: HashMap<String, RefEntry>,
    counter: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chains: Vec<ScopeChain>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    chain_of: BTreeMap<String, usize>,
}

impl From<RefMap> for RefMapWire {
    fn from(map: RefMap) -> Self {
        let RefMap { mut inner, counter } = map;
        let mut chains: Vec<ScopeChain> = Vec::new();
        let mut index_by_ptr: HashMap<(usize, usize), usize> = HashMap::new();
        let mut index_by_value: HashMap<ScopeChain, usize> = HashMap::new();
        let mut chain_of = BTreeMap::new();
        for (ref_id, entry) in inner.iter_mut() {
            if entry.scope.chain.is_empty() {
                continue;
            }
            let chain = std::mem::take(&mut entry.scope.chain);
            let ptr_key = chain.ptr_key();
            let known = ptr_key
                .and_then(|key| index_by_ptr.get(&key).copied())
                .or_else(|| index_by_value.get(&chain).copied());
            let index = match known {
                Some(index) => {
                    if let Some(key) = ptr_key {
                        index_by_ptr.insert(key, index);
                    }
                    index
                }
                None => {
                    let index = chains.len();
                    if let Some(key) = ptr_key {
                        index_by_ptr.insert(key, index);
                    }
                    index_by_value.insert(chain.clone(), index);
                    chains.push(chain);
                    index
                }
            };
            chain_of.insert(ref_id.clone(), index);
        }
        Self {
            inner,
            counter,
            chains,
            chain_of,
        }
    }
}

impl TryFrom<RefMapWire> for RefMap {
    type Error = String;

    fn try_from(wire: RefMapWire) -> Result<Self, Self::Error> {
        let RefMapWire {
            mut inner,
            counter,
            chains,
            chain_of,
        } = wire;
        let mut budget = WireScopeBudget::new();
        let mut shared_chains = Vec::with_capacity(chains.len());
        for chain in &chains {
            shared_chains.push(budget.intern_chain(chain)?);
        }
        for entry in inner.values_mut() {
            if !entry.scope.chain.is_empty() {
                entry.scope.chain = budget.intern_chain(&entry.scope.chain)?;
            }
        }
        for (ref_id, index) in chain_of {
            let chain = shared_chains.get(index).ok_or(MISSING_CHAIN)?;
            let entry = inner.get_mut(&ref_id).ok_or(MISSING_CHAIN)?;
            entry.scope.chain = chain.clone();
        }
        Ok(Self { inner, counter })
    }
}
