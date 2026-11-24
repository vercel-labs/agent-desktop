use crate::{SnapshotSurface, adapter::TreeOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BaselineScope {
    pub(crate) surface: SnapshotSurface,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) root_ref: Option<String>,
    pub(crate) max_depth: u8,
    pub(crate) interactive_only: bool,
    pub(crate) compact: bool,
    pub(crate) include_bounds: bool,
    pub(crate) skeleton: bool,
}

impl BaselineScope {
    pub(crate) fn from_options(opts: &TreeOptions, root_ref: Option<&str>) -> Self {
        Self {
            surface: opts.surface,
            root_ref: root_ref.map(str::to_owned),
            max_depth: opts.max_depth,
            interactive_only: opts.interactive_only,
            compact: opts.compact,
            include_bounds: opts.include_bounds,
            skeleton: opts.skeleton,
        }
    }
}
