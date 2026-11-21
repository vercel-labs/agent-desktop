use serde::{Deserialize, Serialize};

/// The target element's own state, as an action's witness reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetWitness {
    pub selected: Option<bool>,
    pub value: Option<String>,
    /// Open transient surfaces (a menu, a popover) directly below the target.
    pub surfaces: Option<u32>,
}
