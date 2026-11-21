use serde::{Deserialize, Serialize};

use crate::{DeliverySemantics, LaunchGuidance, ProcessId, WindowInfo};

/// What a launch can honestly report. The process starting and the application
/// presenting a window are separate outcomes: a background application never
/// shows one, and a document-based application creates its first window only
/// once it is brought forward. Reporting them separately lets a caller wait for
/// the window it actually asked for instead of a deadline. Run `list-apps` for
/// the presentation of an application that reports no window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResult {
    pub app: String,
    pub pid: ProcessId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowInfo>,
    #[serde(flatten)]
    pub guidance: LaunchGuidance,
    /// What the launch did to the system, set by the adapter that knows:
    /// `not_delivered` when it attached and dispatched nothing,
    /// `delivered_verified` when it read back its postcondition — a process
    /// instance absent before the launch or the target frontmost after an
    /// activation — and `delivered_unverified` otherwise. Payloads written
    /// before the field existed deserialize as `unknown`.
    #[serde(default)]
    pub disposition: DeliverySemantics,
}

#[cfg(test)]
#[path = "launch_result_tests.rs"]
mod tests;
