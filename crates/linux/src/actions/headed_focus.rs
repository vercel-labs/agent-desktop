use agent_desktop_core::{ActionRequest, HeadedFocus};

use crate::system::display::display_server::DisplayServer;

/// A Wayland session lets no client activate a window, so a headed action
/// there focuses nothing and reaches the element through its own route.
pub(crate) fn headed_focus_on(
    server: &DisplayServer,
    request: &ActionRequest,
) -> Option<HeadedFocus> {
    match server {
        DisplayServer::Wayland => None,
        _ => request.headed_focus(),
    }
}

#[cfg(test)]
#[path = "headed_focus_tests.rs"]
mod tests;
