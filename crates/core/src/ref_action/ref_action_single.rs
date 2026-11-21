use crate::{
    ActionRequest, ActionResult, AdapterError, NativeHandle, ref_action::into_adapter_error,
    ref_action::ref_action_context::RefActionContext,
    ref_action::ref_action_wait_context::RefActionWaitContext,
};

pub(crate) fn execute_single_shot(
    context: RefActionWaitContext<'_>,
    request: ActionRequest,
    deadline: crate::Deadline,
    lease: &crate::InteractionLease,
) -> Result<(ActionResult, NativeHandle), AdapterError> {
    let keyboard_is_aimed_at_the_element = request.action.delivered_by_keyboard()
        && !context.adapter.keyboard_needs_window_activation();
    if request.headed_focus().is_some() && !keyboard_is_aimed_at_the_element {
        let focus = context
            .adapter
            .headed_focus_for_ref(context.entry, &request, deadline)
            .map_err(crate::ref_action::mark_pre_dispatch_resolution_failure)?;
        match focus {
            Some(focus) if focus.focuses_application() => {
                let process = context.entry.process.identity().ok_or_else(|| {
                    AdapterError::stale_ref_because("Target process identity is unavailable")
                })?;
                context.adapter.focus_app(process, lease)?;
            }
            Some(_) => {
                crate::actions::headed_focus::focus_entry_window(
                    context.entry,
                    context.adapter,
                    context.context,
                    lease,
                )
                .map_err(crate::ref_action::into_adapter_error)?;
            }
            None => {}
        }
    }
    crate::ref_action::dispatch_resolved_landing(
        RefActionContext::new(context, deadline),
        request,
        lease,
    )
    .map_err(into_adapter_error)
}
