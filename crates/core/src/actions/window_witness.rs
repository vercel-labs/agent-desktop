use serde::{Deserialize, Serialize};

/// The target's window and process, as an action's witness reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowWitness {
    /// The window title, with its document where the platform names one.
    pub title: Option<String>,
    /// A cheap sign of the window's content: equal signs mean that the
    /// content the platform read did not change.
    pub content: Option<u64>,
    /// How many windows the target's process has.
    pub windows: Option<u32>,
    /// Whether the target's window is still one of its process's windows.
    pub present: Option<bool>,
}
