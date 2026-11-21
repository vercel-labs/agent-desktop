use crate::tree::Field;
use crate::tree::properties::grid_read::GridRead;

/// The individually read accessible properties of a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeProperties {
    pub(crate) name: Field<String>,
    pub(crate) description: Field<String>,
    pub(crate) child_count: Field<i32>,
    pub(crate) accessible_id: Field<String>,
    pub(crate) grid: GridRead,
}

impl NodeProperties {
    pub(crate) fn failed(detail: &str) -> Self {
        Self {
            name: Field::Failed(detail.to_owned()),
            description: Field::Failed(detail.to_owned()),
            child_count: Field::Failed(detail.to_owned()),
            accessible_id: Field::Failed(detail.to_owned()),
            grid: GridRead::default(),
        }
    }
}
