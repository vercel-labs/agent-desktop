//! The agent-desktop GNOME Shell extension, the compositor-side half of the
//! adapter on a GNOME Wayland session. A Wayland client cannot read where any
//! window sits or draw above other clients; the extension runs inside
//! gnome-shell, so it reports window rectangles and draws the cursor overlay.
//! Its source is `crates/linux/gnome-shell-extension/`.
use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use zbus::zvariant::{DynamicType, OwnedValue};

use crate::system::bus::session_bus::SessionBus;
use crate::system::{NAME_HAS_NO_OWNER, SERVICE_UNKNOWN, within};

pub(crate) mod extension_cursor;
pub(crate) mod extension_frames;
pub(crate) mod extension_place;
pub(crate) mod extension_request;
pub(crate) mod extension_start;
pub(crate) mod extension_window;

const BUS_NAME: &str = "dev.agentbrowser.AgentDesktop";
const OBJECT_PATH: &str = "/dev/agentbrowser/AgentDesktop";
const INTERFACE: &str = "dev.agentbrowser.AgentDesktop.Shell";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const UNKNOWN_CALLS: [&str; 4] = [
    "org.freedesktop.DBus.Error.UnknownMethod",
    "org.freedesktop.DBus.Error.UnknownInterface",
    "org.freedesktop.DBus.Error.UnknownObject",
    "org.freedesktop.DBus.Error.UnknownProperty",
];

/// The request and reply shapes this build speaks. The extension reports its
/// own as the `Version` property; any other value is refused before a call.
pub(crate) const PROTOCOL_VERSION: u32 = 1;

/// The extension's UUID, as `gnome-extensions` names it.
pub(crate) const EXTENSION_UUID: &str = "agent-desktop@agent-browser.dev";

/// Why the extension could not answer a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unavailable {
    /// Nothing owns the extension's bus name: it is not installed, not
    /// enabled, or the session runs no GNOME Shell.
    Absent,
    /// The owner speaks another protocol version, or not this interface.
    Incompatible,
}

impl Unavailable {
    pub(crate) const fn cause(self) -> &'static str {
        match self {
            Self::Absent => "shell_extension_absent",
            Self::Incompatible => "shell_extension_incompatible",
        }
    }

    pub(crate) fn suggestion(self) -> String {
        match self {
            Self::Absent => format!(
                "Install and enable the {EXTENSION_UUID} GNOME Shell extension, then log out and back in"
            ),
            Self::Incompatible => format!(
                "Install the {EXTENSION_UUID} extension from the same agent-desktop release as this CLI, then log out and back in"
            ),
        }
    }
}

fn classify(error: zbus::Error) -> Result<Unavailable, AdapterError> {
    match &error {
        zbus::Error::MethodError(name, _, _)
            if matches!(name.as_str(), SERVICE_UNKNOWN | NAME_HAS_NO_OWNER) =>
        {
            Ok(Unavailable::Absent)
        }
        zbus::Error::MethodError(name, _, _) if UNKNOWN_CALLS.contains(&name.as_str()) => {
            Ok(Unavailable::Incompatible)
        }
        _ => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "The agent-desktop GNOME Shell extension refused the call",
        )
        .with_platform_detail(error.to_string())),
    }
}

async fn version(connection: &zbus::Connection) -> Result<Result<u32, Unavailable>, AdapterError> {
    let reply = match connection
        .call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some(PROPERTIES),
            "Get",
            &(INTERFACE, "Version"),
        )
        .await
    {
        Ok(reply) => reply,
        Err(error) => return classify(error).map(Err),
    };
    Ok(reply
        .body()
        .deserialize::<OwnedValue>()
        .ok()
        .and_then(|value| value.downcast_ref::<u32>().ok())
        .ok_or(Unavailable::Incompatible))
}

/// Calls one extension method after checking that the extension speaks
/// [`PROTOCOL_VERSION`]. The outer error is a transport or deadline failure;
/// the inner one says the extension is not there to answer, which includes a
/// process with no session bus to find it on.
pub(crate) fn call<B, R>(
    method: &str,
    body: &B,
    deadline: Deadline,
) -> Result<Result<R, Unavailable>, AdapterError>
where
    B: Serialize + DynamicType,
    R: DeserializeOwned + zbus::zvariant::Type,
{
    let connection = match SessionBus::get(deadline) {
        Ok(connection) => connection,
        Err(error) if error.code == ErrorCode::PlatformNotSupported => {
            return Ok(Err(Unavailable::Absent));
        }
        Err(error) => return Err(error),
    };
    within(deadline, async {
        match version(&connection).await? {
            Ok(PROTOCOL_VERSION) => {}
            Ok(_) => return Ok(Err(Unavailable::Incompatible)),
            Err(unavailable) => return Ok(Err(unavailable)),
        }
        let reply = match connection
            .call_method(Some(BUS_NAME), OBJECT_PATH, Some(INTERFACE), method, body)
            .await
        {
            Ok(reply) => reply,
            Err(error) => return classify(error).map(Err),
        };
        reply.body().deserialize::<R>().map(Ok).map_err(|error| {
            AdapterError::internal(
                "The agent-desktop GNOME Shell extension answered malformed data",
            )
            .with_platform_detail(error.to_string())
        })
    })
}

#[cfg(test)]
#[path = "extension_tests.rs"]
mod tests;
