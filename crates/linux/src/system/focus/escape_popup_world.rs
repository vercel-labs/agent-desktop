//! The live desktop the background Escape popup fallback reads and acts on,
//! behind a seam so the identity checks can be driven by a fixed desktop.

use agent_desktop_core::{AdapterError, Deadline, ProcessIdentity, WindowInfo};

use super::{close_verbs, popup_open, verify_element_owner};
use crate::actions::action_call::do_action;
use crate::actions::context::Target;
use crate::actions::write::mutation::WriteReply;
use crate::system::process::process_facts;
use crate::system::process::process_lookup::Lookup;
use crate::system::window::window_ops::windows_of_process;
use crate::system::{REGISTRY_ROOT_PATH, SharedBus, within};
use crate::tree::menu::observe_menu::{rooted, scan_walk_structure};
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind, read_toolkit};
use crate::tree::{AtspiElement, ObservedRead, WalkedNode};

const SCAN_NODES: usize = 600;
const OWNER_NODES: usize = 64;
const POPUP_CONTAINER_CHILDREN: usize = 16;

/// What one popup attempt reads and does about the desktop, so its identity
/// checks can be driven by a fixed desktop.
pub(super) trait PopupWorld {
    fn live(&self, pid: u32) -> Lookup;

    fn windows(&self, pid: u32, deadline: Deadline) -> Result<Vec<WindowInfo>, AdapterError>;

    fn verify_owner(
        &self,
        expected: &ProcessIdentity,
        element: &AtspiElement,
        deadline: Deadline,
    ) -> Result<(), AdapterError>;

    fn walk(
        &self,
        frame: &AtspiElement,
        deadline: Deadline,
    ) -> Result<WalkedNode<ObservedRead>, AdapterError>;

    fn verbs(
        &self,
        owner: &AtspiElement,
        role: Option<u32>,
        deadline: Deadline,
    ) -> Result<(Vec<String>, &'static [&'static str]), AdapterError>;

    fn dismiss(
        &self,
        owner: &AtspiElement,
        names: &[String],
        verbs: &'static [&'static str],
        deadline: Deadline,
    ) -> Result<WriteReply, AdapterError>;

    fn popup_still_open(&self, owner: &AtspiElement, deadline: Deadline) -> Option<bool>;
}

impl PopupWorld for SharedBus {
    fn live(&self, pid: u32) -> Lookup {
        process_facts::lookup(pid)
    }

    fn windows(&self, pid: u32, deadline: Deadline) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(windows_of_process(self, pid, deadline)?.0)
    }

    fn verify_owner(
        &self,
        expected: &ProcessIdentity,
        element: &AtspiElement,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        verify_element_owner(self, expected, element, deadline)
    }

    fn walk(
        &self,
        frame: &AtspiElement,
        deadline: Deadline,
    ) -> Result<WalkedNode<ObservedRead>, AdapterError> {
        scan_walk_structure(
            self,
            frame,
            SCAN_NODES,
            Some(POPUP_CONTAINER_CHILDREN),
            deadline,
        )
    }

    fn verbs(
        &self,
        owner: &AtspiElement,
        role: Option<u32>,
        deadline: Deadline,
    ) -> Result<(Vec<String>, &'static [&'static str]), AdapterError> {
        let verbs = close_verbs(role, owner_toolkit(self, owner, deadline));
        let handle = owner.clone().into_native_handle();
        let target = Target {
            bus: self,
            handle: &handle,
            element: owner,
            deadline,
        };
        let names = target.view()?.action_names().unwrap_or_default().to_vec();
        Ok((names, verbs))
    }

    fn dismiss(
        &self,
        owner: &AtspiElement,
        names: &[String],
        verbs: &'static [&'static str],
        deadline: Deadline,
    ) -> Result<WriteReply, AdapterError> {
        let handle = owner.clone().into_native_handle();
        let target = Target {
            bus: self,
            handle: &handle,
            element: owner,
            deadline,
        };
        do_action(&target, names, verbs)
    }

    fn popup_still_open(&self, owner: &AtspiElement, deadline: Deadline) -> Option<bool> {
        still_open(self, owner, deadline)
    }
}

fn owner_toolkit(bus: &SharedBus, owner: &AtspiElement, deadline: Deadline) -> ToolkitKind {
    let Ok(connection) = bus.get(deadline) else {
        return ToolkitKind::Other;
    };
    let app = AtspiElement::new(owner.bus_name(), REGISTRY_ROOT_PATH, owner.pid());
    match within(deadline, read_toolkit(connection.inner(), &app)) {
        Ok(toolkit) => kind(&toolkit),
        Err(_) => ToolkitKind::Other,
    }
}

fn still_open(bus: &SharedBus, owner: &AtspiElement, deadline: Deadline) -> Option<bool> {
    let walk = scan_walk_structure(
        bus,
        &rooted(owner),
        OWNER_NODES,
        Some(POPUP_CONTAINER_CHILDREN),
        deadline,
    )
    .ok()?;
    popup_open(&walk)
}
