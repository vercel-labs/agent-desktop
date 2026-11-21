use serde::{Deserialize, Serialize};

/// The application's focused element, as an action's witness reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusWitness {
    /// A stable identity of the focused element; `None` when the platform
    /// cannot name it, which leaves focus evidence unread.
    pub element: Option<String>,
    /// Whether the focused element is the target or lies inside it.
    pub inside_target: Option<bool>,
    /// Whether the focused element lies in the target's window.
    pub in_target_window: Option<bool>,
    pub value: Option<String>,
}

impl FocusWitness {
    /// The application answered that nothing has focus. Unlike `default()`,
    /// which is a focus that was not read, this is a read, so focus that
    /// arrives later counts as moved. Use it only for that answer: a read the
    /// application did not answer stays `default()`.
    pub fn nothing_focused() -> Self {
        Self {
            element: Some(String::new()),
            inside_target: Some(false),
            in_target_window: Some(false),
            value: None,
        }
    }
}
