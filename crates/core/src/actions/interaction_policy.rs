use serde::{Deserialize, Serialize};

use crate::{AdapterError, actions::effect::Effect};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionPolicy {
    pub allow_focus_steal: bool,
    pub allow_cursor_move: bool,
}

impl InteractionPolicy {
    pub fn headless() -> Self {
        Self {
            allow_focus_steal: false,
            allow_cursor_move: false,
        }
    }

    pub fn focus_fallback() -> Self {
        Self {
            allow_focus_steal: true,
            allow_cursor_move: false,
        }
    }

    pub fn headed() -> Self {
        Self {
            allow_focus_steal: true,
            allow_cursor_move: true,
        }
    }

    pub fn is_headed(self) -> bool {
        self.allow_focus_steal && self.allow_cursor_move
    }

    /// Refuses `operation` with `POLICY_DENIED`, and nothing sent, when this
    /// policy does not permit `effect`. A clipboard write needs the headed
    /// policy: no narrower policy grants it.
    pub fn require(self, effect: Effect, operation: &str) -> Result<(), AdapterError> {
        let (allowed, what) = match effect {
            Effect::FocusSteal => (self.allow_focus_steal, "takes key focus or raises a window"),
            Effect::CursorMove => (self.allow_cursor_move, "moves the pointer"),
            Effect::ClipboardWrite => (self.is_headed(), "writes the user's clipboard"),
        };
        if allowed {
            return Ok(());
        }
        Err(AdapterError::policy_denied_for_policy(
            format!("{operation} {what}, which the interaction policy does not allow"),
            self,
        )
        .with_details(serde_json::json!({ "effect": effect })))
    }

    pub fn join(self, other: InteractionPolicy) -> InteractionPolicy {
        InteractionPolicy {
            allow_focus_steal: self.allow_focus_steal || other.allow_focus_steal,
            allow_cursor_move: self.allow_cursor_move || other.allow_cursor_move,
        }
    }
}

impl Default for InteractionPolicy {
    fn default() -> Self {
        Self::headless()
    }
}

#[cfg(test)]
#[path = "interaction_policy_tests.rs"]
mod tests;
