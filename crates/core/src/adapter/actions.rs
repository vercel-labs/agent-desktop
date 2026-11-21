use crate::{
    AdapterError, InteractionLease, actions::action_request::ActionRequest,
    actions::action_result::ActionResult, adapter::native_handle::NativeHandle,
};

pub trait ActionOps: Send + Sync {
    /// What a headed action on the element a ref names focuses before it is
    /// delivered.
    fn headed_focus_for_ref(
        &self,
        _entry: &crate::RefEntry,
        request: &ActionRequest,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::HeadedFocus>, AdapterError> {
        Ok(request.headed_focus())
    }

    /// [`ActionOps::headed_focus_for_ref`] for a resolved element.
    fn headed_focus(
        &self,
        _handle: &NativeHandle,
        request: &ActionRequest,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::HeadedFocus>, AdapterError> {
        Ok(request.headed_focus())
    }

    /// The effect the action on this element has beyond the element itself,
    /// and why, such as a popup or a dialog that takes the user's focus. Core
    /// refuses a request whose policy does not permit it with `POLICY_DENIED`,
    /// before any other actionability check, the wait or a scroll, and reports
    /// the reason as `details.kind`.
    fn foreground_effect(
        &self,
        _handle: &NativeHandle,
        _request: &ActionRequest,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::DeclaredEffect>, AdapterError> {
        Ok(None)
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("execute_action"))
    }

    fn scroll_into_view(
        &self,
        handle: &NativeHandle,
        _policy: crate::InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        let _ = (handle, lease);
        Err(AdapterError::not_supported("scroll_into_view"))
    }

    /// Prepare a ref for dispatch. An adapter may return a strictly verified
    /// handle when its own scroll supplies independent evidence of the same
    /// native object's movement. Otherwise the caller resolves the original
    /// ref again; scrolling alone never authorizes rebinding a target.
    fn scroll_ref_into_view(
        &self,
        _entry: &crate::RefEntry,
        handle: &NativeHandle,
        policy: crate::InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<Option<NativeHandle>, AdapterError> {
        self.scroll_into_view(handle, policy, lease)?;
        Ok(None)
    }
}
