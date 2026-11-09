#[cfg(any(target_os = "macos", test))]
use crate::actions::chain::chain_delivery::DeliveryOutcome;
#[cfg(any(target_os = "macos", test))]
use agent_desktop_core::AdapterError;

#[cfg(any(target_os = "macos", test))]
fn activate_candidates<T>(
    children: &[T],
    mut disturbs: impl FnMut(&T) -> bool,
    mut perform: impl FnMut(&T, &str) -> Result<DeliveryOutcome, AdapterError>,
) -> Result<DeliveryOutcome, AdapterError> {
    for child in children {
        if disturbs(child) {
            continue;
        }
        for action in crate::tree::properties::action_list::PRIMARY_ACTIVATION_ACTIONS {
            let outcome = perform(child, action)?;
            if outcome.terminates_chain() {
                return Ok(outcome);
            }
        }
    }
    Ok(DeliveryOutcome::NotDelivered)
}

#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::actions::chain::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;

    const MAX_CANDIDATES: usize = 4;

    /// A row often publishes no activation itself while the cell inside it
    /// does: Finder's sidebar `treeitem` is inert and its `cell` carries
    /// `AXOpen`. Without this the chain falls through to writing selection,
    /// which the row accepts and reports back while the application never
    /// navigates.
    ///
    /// A member that only its container can select is chosen, not pressed
    /// through a control that happens to sit inside it.
    ///
    /// Unverified delivery ends the search too: absence of an observed effect
    /// cannot authorize another mutation on the same child or a sibling.
    pub(crate) fn activate_descendant(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let instant = deadline;
        if crate::actions::select::container_select::element_activates_by_selection(
            element, deadline,
        ) && crate::tree::properties::selection_owner::selects_only_through_container(
            element, instant,
        ) {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        let children = crate::tree::properties::attributes::copy_ax_array_prefix_result(
            element,
            "AXChildren",
            MAX_CANDIDATES,
            instant,
        )
        .ok()
        .flatten()
        .unwrap_or_default();

        super::activate_candidates(
            &children,
            |child| crate::actions::background_effect::descendant_press_disturbs(child, deadline),
            |child, action| {
                crate::actions::ax_helpers::perform_observed_action(child, action, deadline)
            },
        )
    }
}

pub(crate) use imp::activate_descendant;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unverified_descendant_delivery_stops_before_another_action_or_sibling() {
        let mut attempts = Vec::new();
        let outcome = activate_candidates(
            &[1, 2],
            |_| false,
            |child, action| {
                attempts.push((*child, action.to_owned()));
                Ok(if attempts.len() == 1 {
                    DeliveryOutcome::NotDelivered
                } else {
                    DeliveryOutcome::DeliveredUnverified
                })
            },
        )
        .unwrap();
        assert_eq!(outcome, DeliveryOutcome::DeliveredUnverified);
        assert_eq!(attempts, [(1, "AXPress".into()), (1, "AXOpen".into())]);
    }

    #[test]
    fn a_child_whose_press_would_show_a_menu_or_open_a_dialog_is_never_pressed() {
        let mut pressed = Vec::new();
        let outcome = activate_candidates(
            &[1, 2],
            |child| *child == 1,
            |child, _| {
                pressed.push(*child);
                Ok(DeliveryOutcome::DeliveredVerified)
            },
        )
        .unwrap();
        assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
        assert_eq!(pressed, [2]);
    }

    #[test]
    fn unsupported_descendant_actions_can_reach_a_verified_sibling() {
        let mut attempts = 0;
        let outcome = activate_candidates(
            &[1, 2, 3],
            |_| false,
            |child, _| {
                attempts += 1;
                Ok(if *child == 2 {
                    DeliveryOutcome::DeliveredVerified
                } else {
                    DeliveryOutcome::NotDelivered
                })
            },
        )
        .unwrap();
        assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
        assert_eq!(attempts, 4);
    }
}
