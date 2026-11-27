use std::collections::HashMap;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;
use zbus::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use super::key_session::KeySession;
use super::portal_request::{
    DESKTOP_PATH, PORTAL, RD_IFACE, SC_IFACE, SESSION_IFACE, check_response, classify_error,
    parse_stream_node, portal_unavailable, wait_portal_request,
};
use super::portal_token::{load_restore_token, next_handle_token, save_restore_token};
use crate::system::bus::session_bus::{SessionBus, is_transport_failure};
use crate::system::within;

const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const DEV_KEYBOARD: u32 = 1;
const DEV_POINTER: u32 = 2;
const SOURCE_MONITOR: u32 = 1;
const PERSIST_REVOKED: u32 = 2;

/// XDG Desktop Portal RemoteDesktop session, supported by KDE Plasma and other
/// Wayland compositors implementing `org.freedesktop.portal.RemoteDesktop`.
pub(crate) struct PortalRemoteDesktop {
    connection: Connection,
    session: Option<OwnedObjectPath>,
    started: bool,
    stream_node: Option<u32>,
    want_screencast: bool,
}

impl PortalRemoteDesktop {
    pub(crate) fn is_pointer_available() -> bool {
        let Ok(deadline) = Deadline::detached_after(500) else {
            return false;
        };
        Self::open(deadline, true).is_ok()
    }

    pub(crate) fn open(deadline: Deadline, want_screencast: bool) -> Result<Self, AdapterError> {
        let connection = SessionBus::get(deadline)?;
        let required = if want_screencast {
            DEV_POINTER
        } else {
            DEV_KEYBOARD
        };
        let available: u32 = within(deadline, async {
            let reply = connection
                .call_method(
                    Some(PORTAL),
                    DESKTOP_PATH,
                    Some(PROPERTIES),
                    "Get",
                    &(RD_IFACE, "AvailableDeviceTypes"),
                )
                .await
                .map_err(|error| {
                    if is_transport_failure(&error) {
                        SessionBus::forget(&connection);
                        classify_error(&error)
                    } else {
                        portal_unavailable(error)
                    }
                })?;
            let value = reply
                .body()
                .deserialize::<OwnedValue>()
                .map_err(portal_unavailable)?;
            u32::try_from(value).map_err(|_| {
                AdapterError::new(
                    ErrorCode::ActionNotSupported,
                    "The XDG RemoteDesktop portal reported an invalid device mask",
                )
                .with_disposition(DeliverySemantics::not_delivered())
            })
        })?;
        if (available & required) == 0 {
            return Err(AdapterError::new(
                ErrorCode::ActionNotSupported,
                "The XDG RemoteDesktop portal does not advertise the required input device",
            )
            .with_details(json!({ "backend": RD_IFACE, "available_devices": available }))
            .with_disposition(DeliverySemantics::not_delivered()));
        }
        Ok(Self {
            connection,
            session: None,
            started: false,
            stream_node: None,
            want_screencast,
        })
    }

    fn session_path(&self) -> Result<&OwnedObjectPath, AdapterError> {
        self.session.as_ref().ok_or_else(|| {
            AdapterError::internal("The portal remote desktop session was not created")
        })
    }

    pub(crate) fn stream_node(&self) -> Option<u32> {
        self.stream_node
    }

    pub(crate) fn pointer_motion_absolute(
        &mut self,
        x: f64,
        y: f64,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let session = self.session_path()?.clone();
        let node = self.stream_node.ok_or_else(|| {
            AdapterError::new(
                ErrorCode::ActionFailed,
                "No ScreenCast monitor stream is attached for absolute pointer positioning",
            )
            .with_disposition(DeliverySemantics::not_delivered())
        })?;
        let empty: HashMap<&str, Value<'_>> = HashMap::new();
        within(deadline, async {
            self.connection
                .call_method(
                    Some(PORTAL),
                    DESKTOP_PATH,
                    Some(RD_IFACE),
                    "NotifyPointerMotionAbsolute",
                    &(session, empty, node, x, y),
                )
                .await
                .map(|_| ())
                .map_err(|error| classify_error(&error))
        })
    }

    pub(crate) fn pointer_button(
        &mut self,
        button: u32,
        down: bool,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let session = self.session_path()?.clone();
        let code = i32::try_from(button)
            .map_err(|_| AdapterError::internal("The pointer button code is out of range"))?;
        let state = u32::from(down);
        let empty: HashMap<&str, Value<'_>> = HashMap::new();
        within(deadline, async {
            self.connection
                .call_method(
                    Some(PORTAL),
                    DESKTOP_PATH,
                    Some(RD_IFACE),
                    "NotifyPointerButton",
                    &(session, empty, code, state),
                )
                .await
                .map(|_| ())
                .map_err(|error| classify_error(&error))
        })
    }

    pub(crate) fn pointer_axis_discrete(
        &mut self,
        axis: u32,
        steps: i32,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let session = self.session_path()?.clone();
        let empty: HashMap<&str, Value<'_>> = HashMap::new();
        within(deadline, async {
            self.connection
                .call_method(
                    Some(PORTAL),
                    DESKTOP_PATH,
                    Some(RD_IFACE),
                    "NotifyPointerAxisDiscrete",
                    &(session, empty, axis, steps),
                )
                .await
                .map(|_| ())
                .map_err(|error| classify_error(&error))
        })
    }

    fn select_devices(
        &self,
        session: &OwnedObjectPath,
        saved_token: Option<&str>,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let token = next_handle_token("select");
        let (code, _) = within(deadline, async {
            wait_portal_request(&self.connection, &token, || async {
                let mut options: HashMap<&str, Value<'_>> = HashMap::new();
                options.insert("handle_token", Value::from(token.as_str()));
                options.insert("types", Value::from(DEV_KEYBOARD | DEV_POINTER));
                options.insert("persist_mode", Value::from(PERSIST_REVOKED));
                if let Some(restore) = saved_token {
                    options.insert("restore_token", Value::from(restore));
                }
                self.connection
                    .call_method(
                        Some(PORTAL),
                        DESKTOP_PATH,
                        Some(RD_IFACE),
                        "SelectDevices",
                        &(session.clone(), options),
                    )
                    .await?
                    .body()
                    .deserialize::<OwnedObjectPath>()
            })
            .await
        })?;
        check_response(code, "SelectDevices")
    }

    fn select_screencast(
        &self,
        session: &OwnedObjectPath,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let token = next_handle_token("sc");
        let (code, _) = within(deadline, async {
            wait_portal_request(&self.connection, &token, || async {
                let mut options: HashMap<&str, Value<'_>> = HashMap::new();
                options.insert("handle_token", Value::from(token.as_str()));
                options.insert("types", Value::from(SOURCE_MONITOR));
                options.insert("multiple", Value::from(false));
                self.connection
                    .call_method(
                        Some(PORTAL),
                        DESKTOP_PATH,
                        Some(SC_IFACE),
                        "SelectSources",
                        &(session.clone(), options),
                    )
                    .await?
                    .body()
                    .deserialize::<OwnedObjectPath>()
            })
            .await
        })?;
        check_response(code, "SelectSources")
    }
}

impl KeySession for PortalRemoteDesktop {
    fn create(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        let token = next_handle_token("create");
        let session_token = next_handle_token("sess");
        let (code, results) = within(deadline, async {
            wait_portal_request(&self.connection, &token, || async {
                let mut options: HashMap<&str, Value<'_>> = HashMap::new();
                options.insert("handle_token", Value::from(token.as_str()));
                options.insert("session_handle_token", Value::from(session_token.as_str()));
                self.connection
                    .call_method(
                        Some(PORTAL),
                        DESKTOP_PATH,
                        Some(RD_IFACE),
                        "CreateSession",
                        &(options,),
                    )
                    .await?
                    .body()
                    .deserialize::<OwnedObjectPath>()
            })
            .await
        })?;
        check_response(code, "CreateSession")?;
        let handle_str = results
            .get("session_handle")
            .and_then(|value| String::try_from(value.clone()).ok())
            .ok_or_else(|| {
                AdapterError::new(
                    ErrorCode::ActionFailed,
                    "CreateSession succeeded without returning a session_handle",
                )
            })?;
        let path = OwnedObjectPath::try_from(handle_str).map_err(|error| {
            AdapterError::new(
                ErrorCode::ActionFailed,
                "The portal returned an invalid session_handle path",
            )
            .with_platform_detail(error.to_string())
        })?;
        self.session = Some(path);
        Ok(())
    }

    fn start(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        let session = self.session_path()?.clone();
        let saved_token = load_restore_token(self.want_screencast);
        self.select_devices(&session, saved_token.as_deref(), deadline)?;
        if self.want_screencast {
            self.select_screencast(&session, deadline)?;
        }
        let start_token = next_handle_token("start");
        let start_deadline = Deadline::detached_after(30_000).unwrap_or(deadline);
        let (start_code, results) = within(start_deadline, async {
            wait_portal_request(&self.connection, &start_token, || async {
                let mut options: HashMap<&str, Value<'_>> = HashMap::new();
                options.insert("handle_token", Value::from(start_token.as_str()));
                self.connection
                    .call_method(
                        Some(PORTAL),
                        DESKTOP_PATH,
                        Some(RD_IFACE),
                        "Start",
                        &(session.clone(), "", options),
                    )
                    .await?
                    .body()
                    .deserialize::<OwnedObjectPath>()
            })
            .await
        })?;
        check_response(start_code, "Start")?;
        if let Some(restore) = results
            .get("restore_token")
            .and_then(|value| String::try_from(value.clone()).ok())
        {
            save_restore_token(self.want_screencast, &restore);
        }
        self.stream_node = parse_stream_node(&results);
        self.started = true;
        Ok(())
    }

    fn keysym(&mut self, keysym: u32, down: bool, deadline: Deadline) -> Result<(), AdapterError> {
        let session = self.session_path()?.clone();
        let sym = i32::try_from(keysym)
            .map_err(|_| AdapterError::internal("The keysym is outside the portal range"))?;
        let state = u32::from(down);
        let empty: HashMap<&str, Value<'_>> = HashMap::new();
        within(deadline, async {
            self.connection
                .call_method(
                    Some(PORTAL),
                    DESKTOP_PATH,
                    Some(RD_IFACE),
                    "NotifyKeyboardKeysym",
                    &(session, empty, sym, state),
                )
                .await
                .map(|_| ())
                .map_err(|error| classify_error(&error))
        })
    }

    fn stop(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        let Some(session) = self.session.take() else {
            self.started = false;
            return Ok(());
        };
        self.started = false;
        self.stream_node = None;
        within(deadline, async {
            let _ = self
                .connection
                .call_method(
                    Some(PORTAL),
                    session.as_str(),
                    Some(SESSION_IFACE),
                    "Close",
                    &(),
                )
                .await;
            Ok(())
        })
    }

    fn resumed(&self) -> bool {
        self.started
    }

    fn retain(&self) -> bool {
        self.started
    }
}
