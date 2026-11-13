use agent_desktop_core::{AdapterError, SurfaceOpening, WindowInfo};

/// A surface found presented is returned as it stands and never raised. A
/// surface that was not presented is raised, and the window the raise is
/// observed to present is the answer, so `Raised` always carries an observation
/// rather than the fact that the raise was sent.
pub(super) fn opening_of(
    presented: Option<WindowInfo>,
    raise: impl FnOnce() -> Result<WindowInfo, AdapterError>,
) -> Result<SurfaceOpening, AdapterError> {
    match presented {
        Some(window) => Ok(SurfaceOpening::Present(window)),
        None => raise().map(SurfaceOpening::Raised),
    }
}

#[cfg(test)]
#[path = "shell_surface_opening_tests.rs"]
mod tests;
