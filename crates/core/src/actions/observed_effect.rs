use serde::{Deserialize, Serialize};

/// What a before-and-after read of an action's witness showed.
///
/// `NotObserved` with a delivered disposition does not mean that nothing
/// happened: an effect can come later, or in another process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedEffect {
    /// A change that only the action explains: on the target, under it, or
    /// focus moving into it.
    Observed,
    /// No change in anything the witness could read.
    NotObserved,
    /// Only a change that the application could also make by itself, such as
    /// a new window title.
    Indeterminate,
}
