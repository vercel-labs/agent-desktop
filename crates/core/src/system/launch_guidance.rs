use serde::{Deserialize, Serialize};

use crate::{system::cdp_endpoint::CdpEndpoint, tree::renderer_kind::RendererKind};

/// What a launch tells the calling agent about how to drive the application.
/// It serializes flattened into the launch result, so each key sits beside
/// `app` and `pid` and is left out when absent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaunchGuidance {
    /// The verified DevTools endpoint, present only when `--cdp` was
    /// requested and the launched process answered `/json/version`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdp: Option<CdpEndpoint>,
    /// The application's renderer, detected best-effort from its bundle.
    /// `Chromium` means Electron or CEF; absent means undetected, not
    /// "not Chromium" — detection failure never fails the launch itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer: Option<RendererKind>,
    /// Guidance for the calling agent on how to drive this application,
    /// set by core from `renderer` and whether `cdp` is present. Absent
    /// when there is nothing more useful to say than the fields already do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}
