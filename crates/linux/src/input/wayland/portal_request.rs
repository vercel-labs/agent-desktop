use std::collections::HashMap;

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use futures_util::StreamExt;
use serde_json::json;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, Proxy};

use super::portal_token::request_path;
use crate::system::{NAME_HAS_NO_OWNER, SERVICE_UNKNOWN};

pub(super) const PORTAL: &str = "org.freedesktop.portal.Desktop";
pub(super) const DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
pub(super) const RD_IFACE: &str = "org.freedesktop.portal.RemoteDesktop";
pub(super) const SC_IFACE: &str = "org.freedesktop.portal.ScreenCast";
pub(super) const SESSION_IFACE: &str = "org.freedesktop.portal.Session";
const REQUEST_IFACE: &str = "org.freedesktop.portal.Request";

pub(super) fn portal_unavailable(error: zbus::Error) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "The XDG RemoteDesktop portal is not available in this Wayland session",
    )
    .with_platform_detail(error.to_string())
    .with_suggestion("Install and run xdg-desktop-portal with a backend supporting RemoteDesktop")
    .with_details(json!({ "backend": RD_IFACE, "physical_delivery_started": false }))
    .with_disposition(DeliverySemantics::not_delivered())
}

pub(super) fn classify_error(error: &zbus::Error) -> AdapterError {
    let missing = matches!(
        error,
        zbus::Error::MethodError(name, _, _)
            if name.as_str() == SERVICE_UNKNOWN || name.as_str() == NAME_HAS_NO_OWNER
    );
    if missing {
        return AdapterError::new(
            ErrorCode::ActionNotSupported,
            "The XDG RemoteDesktop portal is not running in this Wayland session",
        )
        .with_platform_detail(error.to_string())
        .with_disposition(DeliverySemantics::not_delivered());
    }
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The XDG RemoteDesktop portal call failed",
    )
    .with_platform_detail(error.to_string())
}

pub(super) fn parse_stream_node(results: &HashMap<String, OwnedValue>) -> Option<u32> {
    let streams = results.get("streams")?;
    let array = zbus::zvariant::Array::try_from(streams.clone()).ok()?;
    let first = array.first()?;
    let structure = zbus::zvariant::Structure::try_from(first.clone()).ok()?;
    let field = structure.fields().first()?;
    u32::try_from(field.clone()).ok()
}

fn unique_sender(connection: &Connection) -> String {
    connection
        .unique_name()
        .map(|name| name.to_string())
        .unwrap_or_default()
}

pub(super) async fn wait_portal_request<F, Fut>(
    connection: &Connection,
    token: &str,
    call: F,
) -> Result<(u32, HashMap<String, OwnedValue>), AdapterError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<OwnedObjectPath, zbus::Error>>,
{
    let expected = request_path(&unique_sender(connection), token);
    let proxy = Proxy::new(connection, PORTAL, expected.clone(), REQUEST_IFACE)
        .await
        .map_err(|error| classify_error(&error))?;
    let mut stream = proxy
        .receive_signal("Response")
        .await
        .map_err(|error| classify_error(&error))?;
    let handle = call().await.map_err(|error| classify_error(&error))?;
    if handle.as_str() != expected {
        let alt = Proxy::new(connection, PORTAL, handle.as_str(), REQUEST_IFACE)
            .await
            .map_err(|error| classify_error(&error))?;
        stream = alt
            .receive_signal("Response")
            .await
            .map_err(|error| classify_error(&error))?;
    }
    let message = stream.next().await.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "The RemoteDesktop portal request ended with no response",
        )
    })?;
    message
        .body()
        .deserialize::<(u32, HashMap<String, OwnedValue>)>()
        .map_err(|error| classify_error(&error))
}

pub(super) fn check_response(code: u32, step: &str) -> Result<(), AdapterError> {
    if code == 0 {
        return Ok(());
    }
    if code == 1 {
        return Err(AdapterError::new(
            ErrorCode::PermDenied,
            format!("The desktop portal denied remote input permission during {step}"),
        )
        .with_suggestion("Approve the desktop's Remote Control permission dialog to allow input")
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    Err(AdapterError::new(
        ErrorCode::PermDenied,
        format!("The desktop portal ended {step} without permission (response {code})"),
    )
    .with_disposition(DeliverySemantics::not_delivered()))
}
