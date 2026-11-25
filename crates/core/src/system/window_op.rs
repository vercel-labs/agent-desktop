use serde::{Deserialize, Serialize};

use crate::{AdapterError, Effect, InteractionPolicy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowOp {
    Resize { width: f64, height: f64 },
    Move { x: f64, y: f64 },
    Minimize,
    Maximize,
    Restore,
}

impl WindowOp {
    /// Un-minimizing raises the window and, on every platform, an application
    /// can activate itself while it comes back, so no policy that keeps the
    /// user's focus lets it run. This is the one refusal the platform adapters
    /// share: `POLICY_DENIED`, nothing sent, a suggestion that names `--headed`.
    pub fn require_unminimize_allowed(policy: InteractionPolicy) -> Result<(), AdapterError> {
        policy.require(Effect::FocusSteal, "restore of a minimized window")
    }
}

#[cfg(test)]
#[path = "window_op_tests.rs"]
mod tests;
