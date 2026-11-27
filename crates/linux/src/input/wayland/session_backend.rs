use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use super::key_session::KeySession;
use super::portal_remote_desktop::PortalRemoteDesktop;
use super::remote_desktop::RemoteDesktop;

/// Active Wayland keyboard session: Mutter RemoteDesktop on GNOME, or the
/// standard XDG RemoteDesktop portal on KDE Plasma and other compositors.
pub(crate) enum WaylandSession {
    Mutter(RemoteDesktop),
    Portal(PortalRemoteDesktop),
}

impl WaylandSession {
    pub(crate) fn open(deadline: Deadline) -> Result<Self, AdapterError> {
        match RemoteDesktop::open(deadline) {
            Ok(mutter) => Ok(Self::Mutter(mutter)),
            Err(mutter_err)
                if matches!(
                    mutter_err.code,
                    ErrorCode::PlatformNotSupported | ErrorCode::ActionNotSupported
                ) =>
            {
                match PortalRemoteDesktop::open(deadline, false) {
                    Ok(portal) => Ok(Self::Portal(portal)),
                    Err(_) => Err(mutter_err),
                }
            }
            Err(error) => Err(error),
        }
    }
}

impl KeySession for WaylandSession {
    fn create(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        match self {
            Self::Mutter(session) => session.create(deadline),
            Self::Portal(session) => session.create(deadline),
        }
    }

    fn start(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        match self {
            Self::Mutter(session) => session.start(deadline),
            Self::Portal(session) => session.start(deadline),
        }
    }

    fn keysym(&mut self, keysym: u32, down: bool, deadline: Deadline) -> Result<(), AdapterError> {
        match self {
            Self::Mutter(session) => session.keysym(keysym, down, deadline),
            Self::Portal(session) => session.keysym(keysym, down, deadline),
        }
    }

    fn stop(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        match self {
            Self::Mutter(session) => session.stop(deadline),
            Self::Portal(session) => session.stop(deadline),
        }
    }

    fn resumed(&self) -> bool {
        match self {
            Self::Mutter(session) => session.resumed(),
            Self::Portal(session) => session.resumed(),
        }
    }

    fn retain(&self) -> bool {
        match self {
            Self::Mutter(session) => session.retain(),
            Self::Portal(session) => session.retain(),
        }
    }
}
