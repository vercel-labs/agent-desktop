use super::baseline_identity::BaselineIdentity;
use super::baseline_presentation::BaselinePresentation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BaselineNode {
    pub(crate) path: Vec<usize>,
    pub(crate) role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) local_ref: Option<String>,
    #[serde(flatten)]
    pub(crate) identity: BaselineIdentity,
    #[serde(flatten)]
    pub(crate) presentation: BaselinePresentation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) children_count: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) subtree_truncated: bool,
}
