use agent_desktop_core::AdapterError;

pub(crate) fn headed_focus(
    request: &agent_desktop_core::ActionRequest,
    classify: impl FnOnce() -> Result<bool, AdapterError>,
) -> Result<Option<agent_desktop_core::HeadedFocus>, AdapterError> {
    use agent_desktop_core::{Action, HeadedFocus};
    if request.policy.is_headed()
        && matches!(
            request.action,
            Action::Click | Action::RightClick | Action::DoubleClick | Action::TripleClick
        )
        && classify()?
    {
        return Ok(Some(if matches!(request.action, Action::Click) {
            HeadedFocus::Application
        } else {
            HeadedFocus::ApplicationAndCursor
        }));
    }
    Ok(request.headed_focus())
}

#[cfg(test)]
#[path = "native_menu_tests.rs"]
mod tests;
