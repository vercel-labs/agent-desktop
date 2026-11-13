use crate::{DeliverySemantics, WindowInfo};

/// What an adapter did to produce a shell surface.
///
/// `Present` means the surface was already presented and the adapter raised
/// nothing, so a retry cannot repeat an effect. `Raised` means the adapter
/// raised it and then observed it presented.
#[derive(Debug, Clone)]
pub enum SurfaceOpening {
    Present(WindowInfo),
    Raised(WindowInfo),
}

impl SurfaceOpening {
    pub fn window(&self) -> &WindowInfo {
        match self {
            Self::Present(window) | Self::Raised(window) => window,
        }
    }

    pub fn into_window(self) -> WindowInfo {
        match self {
            Self::Present(window) | Self::Raised(window) => window,
        }
    }

    pub const fn disposition(&self) -> DeliverySemantics {
        match self {
            Self::Present(_) => DeliverySemantics::not_delivered(),
            Self::Raised(_) => DeliverySemantics::delivered_verified(),
        }
    }
}

#[cfg(test)]
#[path = "surface_opening_tests.rs"]
mod tests;
