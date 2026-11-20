use agent_desktop_core::{Deadline, ObservationRoot};

use crate::system::{REGISTRY_ROOT_PATH, SharedBus, window::window_id, within};
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind, read_toolkit};
use crate::tree::{AtspiElement, atspi_element};

/// The bus guid the refs of an observation are tokened under, or empty for
/// none. A token is issued only for a toolkit whose object paths were measured
/// never to be handed to a new object (GTK3, GTK4 and Chromium, L17-15); Qt
/// gave a dead path to a new object with the same role and name, so a token
/// could name a replacement there, and its refs keep the capture proof.
pub(crate) fn guid_for(bus: &SharedBus, root: &ObservationRoot<'_>, deadline: Deadline) -> String {
    let app = match root {
        ObservationRoot::Window(window) => window_id::decode(&window.id)
            .ok()
            .map(|(bus_name, _)| AtspiElement::new(bus_name, REGISTRY_ROOT_PATH, window.pid.get())),
        ObservationRoot::Element { handle, .. } => atspi_element(handle).ok().map(|element| {
            AtspiElement::new(element.bus_name(), REGISTRY_ROOT_PATH, element.pid())
        }),
        ObservationRoot::Application { .. } => None,
    };
    let Some(app) = app else {
        return String::new();
    };
    let Ok(connection) = bus.get(deadline) else {
        return String::new();
    };
    let connection = connection.inner();
    match within(deadline, read_toolkit(connection, &app)) {
        Ok(toolkit)
            if matches!(
                kind(&toolkit),
                ToolkitKind::Gtk3 | ToolkitKind::Gtk4 | ToolkitKind::Chromium
            ) =>
        {
            connection.server_guid().to_string()
        }
        _ => String::new(),
    }
}
