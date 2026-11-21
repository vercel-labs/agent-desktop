use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RefScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_ref: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub path_is_absolute: bool,
    #[serde(default, skip_serializing_if = "smallvec::SmallVec::is_empty")]
    pub path: crate::refs::RefPath,
    /// The identities of the ancestors that scope an element whose own proof is
    /// not unique in its window, nearest first. Each is unique inside the next,
    /// and the last is unique in the whole window.
    #[serde(default, skip_serializing_if = "crate::ScopeChain::is_empty")]
    pub chain: crate::ScopeChain,
}

fn is_false(value: &bool) -> bool {
    !*value
}
