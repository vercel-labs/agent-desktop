use std::collections::HashMap;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use futures_util::StreamExt;
use serde_json::json;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, Proxy};

use super::portal_token::request_path;
use crate::system::{NAME_HAS_NO_OWNER, SERVICE_UNKNOWN, within};

pub(super) const PORTAL: &str = "org.freedesktop.portal.Desktop";
pub(super) const DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
pub(super) const RD_IFACE: &str = "org.freedesktop.portal.RemoteDesktop";
pub(super) const SC_IFACE: &str = "org.freedesktop.portal.ScreenCast";
pub(super) const SESSION_IFACE: &str = "org.freedesktop.portal.Session";
const REQUEST_IFACE: &str = "org.freedesktop.portal.Request";
const CLOSE_GRACE_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StreamGeometry {
    pub(crate) node_id: u32,
    pub(crate) origin_x: f64,
    pub(crate) origin_y: f64,
    pub(crate) width: Option<f64>,
    pub(crate) height: Option<f64>,
}

impl StreamGeometry {
    pub(crate) fn to_local(
        &self,
        global_x: f64,
        global_y: f64,
    ) -> Result<(f64, f64), AdapterError> {
        let local_x = global_x - self.origin_x;
        let local_y = global_y - self.origin_y;
        if let (Some(width), Some(height)) = (self.width, self.height)
            && (local_x < 0.0 || local_y < 0.0 || local_x > width || local_y > height)
        {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The pointer target is outside the shared ScreenCast monitor",
            )
            .with_details(json!({
                "global_x": global_x,
                "global_y": global_y,
                "stream_origin": [self.origin_x, self.origin_y],
                "stream_size": [width, height],
                "physical_delivery_started": false,
            }))
            .with_disposition(DeliverySemantics::not_delivered()));
        }
        Ok((local_x, local_y))
    }
}

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

fn parse_i32_pair(value: &OwnedValue) -> Option<(f64, f64)> {
    let pair: (i32, i32) = value.clone().try_into().ok()?;
    Some((f64::from(pair.0), f64::from(pair.1)))
}

pub(super) fn parse_stream(results: &HashMap<String, OwnedValue>) -> Option<StreamGeometry> {
    let streams = results.get("streams")?;
    let array = zbus::zvariant::Array::try_from(streams.clone()).ok()?;
    let first = array.first()?;
    let structure = zbus::zvariant::Structure::try_from(first.clone()).ok()?;
    let fields = structure.fields();
    let node_id = u32::try_from(fields.first()?.clone()).ok()?;
    let props: HashMap<String, OwnedValue> = fields
        .get(1)
        .and_then(|field| field.clone().try_into().ok())
        .unwrap_or_default();
    let (origin_x, origin_y) = props
        .get("position")
        .and_then(parse_i32_pair)
        .unwrap_or((0.0, 0.0));
    let (width, height) = props
        .get("size")
        .and_then(parse_i32_pair)
        .filter(|(w, h)| *w > 0.0 && *h > 0.0)
        .map_or((None, None), |(w, h)| (Some(w), Some(h)));
    Some(StreamGeometry {
        node_id,
        origin_x,
        origin_y,
        width,
        height,
    })
}

fn unique_sender(connection: &Connection) -> String {
    connection
        .unique_name()
        .map(|name| name.to_string())
        .unwrap_or_default()
}

pub(super) fn close_request_quietly(connection: &Connection, path: &str) {
    let Ok(grace) = Deadline::detached_after(CLOSE_GRACE_MS) else {
        return;
    };
    let path = path.to_owned();
    let _ = within(grace, async {
        let _ = connection
            .call_method(
                Some(PORTAL),
                path.as_str(),
                Some(REQUEST_IFACE),
                "Close",
                &(),
            )
            .await;
        Ok(())
    });
}

pub(super) fn run_portal_request<F, Fut>(
    connection: &Connection,
    token: &str,
    deadline: Deadline,
    call: F,
) -> Result<(u32, HashMap<String, OwnedValue>), AdapterError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<OwnedObjectPath, zbus::Error>>,
{
    let expected = request_path(&unique_sender(connection), token);
    let outcome = within(deadline, async {
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
    });
    if let Err(error) = &outcome
        && error.code == ErrorCode::Timeout
    {
        close_request_quietly(connection, &expected);
    }
    outcome
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

#[cfg(test)]
#[path = "portal_request_tests.rs"]
mod tests;
