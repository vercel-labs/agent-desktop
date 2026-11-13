use agent_desktop_core::{AdapterError, DeliveryObservation, DeliveryOutcome};

use crate::actions::DeliveryTracker;

/// `AXPress` on a row or a cell means "select me", and a grid that is not the
/// key view accepts it, answers `kAXErrorSuccess`, and selects nothing. The
/// responder is not lying about reaching the application, only about what
/// happened there, so reading the selection back is what tells the two apart.
/// A selection that is still false is evidence that the press changed nothing,
/// so the container's selection is written inside the same rung instead of
/// reporting a success the caller cannot see. The press stays on record when
/// that write sends nothing or cannot be verified. An element that publishes
/// no selection at all is unchanged: absence of evidence ends the chain with
/// the unverified press.
pub(crate) fn press_then_select(
    press: impl FnOnce() -> Result<DeliveryOutcome, AdapterError>,
    read_selection: impl FnOnce() -> Option<bool>,
    select_within_container: impl FnOnce() -> Result<DeliveryOutcome, AdapterError>,
) -> Result<DeliveryOutcome, AdapterError> {
    let pressed = press()?;
    if pressed != DeliveryOutcome::Delivered(DeliveryObservation::Unverified) {
        return Ok(pressed);
    }
    match read_selection() {
        Some(true) => Ok(DeliveryOutcome::Delivered(DeliveryObservation::Verified)),
        None => Ok(pressed),
        Some(false) => match select_within_container()
            .map_err(|error| DeliveryTracker::from_delivered_units(1).annotate(error))?
        {
            selected @ DeliveryOutcome::Delivered(DeliveryObservation::Verified) => Ok(selected),
            _ => Ok(pressed),
        },
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline, DeliveryOutcome};

    use crate::tree::AXElement;

    pub(crate) fn press_or_select_container(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        super::press_then_select(
            || crate::actions::ax_helpers::perform_observed_action(element, "AXPress", deadline),
            || selection_after_press(element, deadline),
            || crate::actions::select::container_select::select_within_container(element, deadline),
        )
    }

    /// A selection appears a moment after the press that caused it. Reading it
    /// once calls a press that worked an effectless one, and the container
    /// then gets a selection that was already on its way. The wait is the same
    /// bounded one the container write uses, and it ends the instant the
    /// selection turns on.
    fn selection_after_press(element: &AXElement, deadline: Deadline) -> Option<bool> {
        if !crate::actions::select::container_select::element_activates_by_selection(
            element, deadline,
        ) {
            return None;
        }
        let read = || {
            crate::tree::properties::attributes::copy_bool_attr_result(
                element,
                crate::actions::select::container_select::SELECTED,
                deadline,
            )
            .ok()
            .flatten()
        };
        let mut observed = None;
        crate::actions::select::container_select::settled(deadline, || {
            observed = read();
            observed != Some(false)
        });
        observed
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::press_or_select_container;

#[cfg(test)]
#[path = "press_selection_tests.rs"]
mod tests;
