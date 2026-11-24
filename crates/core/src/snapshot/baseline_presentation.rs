use crate::{NodeDescriptor, Rect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BaselinePresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) hint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) states: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) available_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) bounds: Option<Rect>,
    #[serde(flatten)]
    pub(crate) descriptors: NodeDescriptor,
}
