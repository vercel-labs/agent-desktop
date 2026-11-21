use serde::{Deserialize, Serialize};

/// What an operation does to the user's foreground session. The interaction
/// policy decides each one through [`crate::InteractionPolicy::require`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// Takes key focus, raises a window, activates an application or opens a
    /// popup over the user's window.
    FocusSteal,
    /// Moves the pointer.
    CursorMove,
    /// Writes the user's clipboard.
    ClipboardWrite,
}
