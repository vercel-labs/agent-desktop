use agent_desktop_core::{
    ActionOps, ActionRequest, ActionResult, AdapterError, Deadline, InteractionLease, NativeHandle,
};

use crate::actions::headed_focus::headed_focus_on;
use crate::adapter::LinuxAdapter;
use crate::system::display::display_server::session_server;

impl LinuxAdapter {
    fn deliver(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        let target = crate::tree::atspi_element(handle)
            .ok()
            .map(|element| element.pid());
        let result = match crate::system::tray::target_of(handle) {
            Some(target) => crate::system::tray::execute(target, request, lease),
            None => {
                let outcome = crate::tree::atspi_element(handle)
                    .ok()
                    .filter(|element| crate::system::shell::is_shell_element(element))
                    .and_then(|element| {
                        crate::system::shell::deliver::deliver(self.bus(), element, &request, lease)
                    });
                match outcome {
                    Some(result) => result,
                    None => crate::actions::execute_action(self.bus(), handle, request, lease),
                }
            }
        };
        crate::system::interaction::action_clock::mark_outcome(target, &result);
        result
    }
}

impl ActionOps for LinuxAdapter {
    fn headed_focus_for_ref(
        &self,
        _entry: &agent_desktop_core::RefEntry,
        request: &ActionRequest,
        _deadline: Deadline,
    ) -> Result<Option<agent_desktop_core::HeadedFocus>, AdapterError> {
        Ok(headed_focus_on(&session_server(), request))
    }

    fn headed_focus(
        &self,
        _handle: &NativeHandle,
        request: &ActionRequest,
        _deadline: Deadline,
    ) -> Result<Option<agent_desktop_core::HeadedFocus>, AdapterError> {
        Ok(headed_focus_on(&session_server(), request))
    }

    fn foreground_effect(
        &self,
        handle: &NativeHandle,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<Option<agent_desktop_core::DeclaredEffect>, AdapterError> {
        crate::actions::foreground::foreground_effect(self.bus(), handle, request, deadline)
    }

    fn scroll_ref_into_view(
        &self,
        entry: &agent_desktop_core::RefEntry,
        handle: &NativeHandle,
        _policy: agent_desktop_core::InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<Option<NativeHandle>, AdapterError> {
        crate::actions::scroll::scroll_ref::scroll(self.bus(), entry, handle, lease)
    }

    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.deliver(handle, request, lease)
    }

    fn refuse_before_scroll(
        &self,
        handle: &NativeHandle,
        request: &ActionRequest,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        crate::actions::refuse_before_scroll(self.bus(), handle, request, lease)
    }

    fn scroll_into_view(
        &self,
        handle: &NativeHandle,
        _policy: agent_desktop_core::InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        crate::actions::before_pointer(crate::actions::scroll_into_view(self.bus(), handle, lease))
    }
}
