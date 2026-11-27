//! Fallback dropdown-button opener for ComboBox controls without ExpandCollapse.

#[cfg(target_os = "windows")]
mod imp {
    use std::time::{Duration, Instant};

    use agent_desktop_core::{AdapterError, Deadline};
    use uiautomation::patterns::UIInvokePattern;
    use uiautomation::types::ControlType;

    use crate::actions::chain::{INVOKE_LABEL, capped_verification_end};
    use crate::actions::mutation::{classify_success, classify_write};
    use crate::actions::select_search::find_named_selection_item;
    use crate::tree::element::UIAElement;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use crate::tree::walk::walker::TreeSource;
    use crate::tree::walk::walker_source::UiaTreeSource;

    const DROPDOWN_WAIT_TIMEOUT: Duration = Duration::from_millis(600);
    const POLL_SLICE: Duration = Duration::from_millis(25);

    pub(crate) fn is_combobox(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::ControlType)
            .number()
            .and_then(|id| ControlType::try_from(id).ok())
            == Some(ControlType::ComboBox)
    }

    pub(crate) fn find_combobox_invoke_button(element: &UIAElement) -> Option<UIAElement> {
        let source = UiaTreeSource::for_root(element).ok()?;
        let prepared = source.prepare_root(element).ok()?;
        let mut child = source.first_child(&prepared).ok();
        while let Some(current) = child {
            let is_button = read_one(&current, TreeProperty::ControlType)
                .number()
                .and_then(|id| ControlType::try_from(id).ok())
                == Some(ControlType::Button);
            if is_button && read_one(&current, TreeProperty::InvokeAvailable).flag() == Some(true) {
                return Some(current);
            }
            child = source.next_sibling(&current).ok();
        }
        None
    }

    pub(crate) fn invoke_dropdown_button(button: &UIAElement) -> Result<(), AdapterError> {
        match button.0.get_pattern::<UIInvokePattern>() {
            Ok(pattern) => match pattern.invoke() {
                Ok(()) => {
                    let _ = classify_success()?;
                    Ok(())
                }
                Err(error) => {
                    let _ = classify_write("Invoke", INVOKE_LABEL, &error)?;
                    Ok(())
                }
            },
            Err(error) => {
                let _ = classify_write("get_pattern", INVOKE_LABEL, &error)?;
                Ok(())
            }
        }
    }

    pub(crate) fn wait_for_selection_item(
        element: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let end = capped_verification_end(deadline, DROPDOWN_WAIT_TIMEOUT)?;
        loop {
            if find_named_selection_item(element, value, deadline, None)?.is_some() {
                return Ok(());
            }
            if Instant::now() >= end {
                return Ok(());
            }
            std::thread::sleep(deadline.remaining_slice(POLL_SLICE)?);
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::{
    find_combobox_invoke_button, invoke_dropdown_button, is_combobox, wait_for_selection_item,
};
