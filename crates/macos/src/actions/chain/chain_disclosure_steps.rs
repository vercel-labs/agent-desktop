#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::chain::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;
    use agent_desktop_core::{AdapterError, Deadline};
    use std::time::{Duration, Instant};

    pub(crate) fn press_to_expand(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        set_disclosure(element, true, deadline)
    }

    pub(crate) fn press_to_collapse(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        set_disclosure(element, false, deadline)
    }

    fn set_disclosure(
        element: &AXElement,
        expanded: bool,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let (satisfied, allow_toggle) =
            disclosure_plan(disclosed_state(element, deadline)?, expanded);
        if satisfied {
            return Ok(DeliveryOutcome::SatisfiedNoDelivery);
        }
        let action = if expanded { "AXExpand" } else { "AXCollapse" };
        prepare(element, deadline)?;
        if crate::actions::ax_helpers::try_ax_action_or_err(element, action, deadline)? {
            let outcome = verify_disclosure(element, expanded, deadline).map_err(after_delivery)?;
            if let Some(delivered) = stop_if_delivered(outcome) {
                return Ok(delivered);
            }
        }
        prepare(element, deadline)?;
        if crate::actions::ax_helpers::is_attr_settable(element, "AXExpanded", deadline)? {
            prepare(element, deadline)?;
            if crate::actions::ax_helpers::set_ax_bool_or_err(
                element,
                "AXExpanded",
                expanded,
                deadline,
            )? {
                let outcome =
                    verify_disclosure(element, expanded, deadline).map_err(after_delivery)?;
                if let Some(delivered) = stop_if_delivered(outcome) {
                    return Ok(delivered);
                }
            }
        }
        if allow_toggle && disclosed_state(element, deadline)? == Some(!expanded) {
            prepare(element, deadline)?;
            if crate::actions::ax_helpers::try_ax_action_or_err(element, "AXPress", deadline)? {
                let outcome =
                    verify_disclosure(element, expanded, deadline).map_err(after_delivery)?;
                if let Some(delivered) = stop_if_delivered(outcome) {
                    return Ok(delivered);
                }
            }
            if let Some(triangle) = disclosure_triangle(element, deadline)? {
                return press_triangle(element, &triangle, expanded, deadline);
            }
        }
        Ok(DeliveryOutcome::NotDelivered)
    }

    /// An outline row publishes its disclosure state but no action that
    /// changes it (`AXDisclosing` write -25200, `AXExpanded` -25205,
    /// M-W0-9); the triangle in its first cell expands it with `AXPress`.
    fn disclosure_triangle(
        row: &AXElement,
        deadline: Deadline,
    ) -> Result<Option<AXElement>, AdapterError> {
        use crate::tree::surface_read;
        let is_triangle = |element: &AXElement| -> Result<bool, AdapterError> {
            Ok(
                surface_read::string(element, "AXRole", deadline)?.as_deref()
                    == Some("AXDisclosureTriangle"),
            )
        };
        for cell in surface_read::elements(row, "AXChildren", deadline)?
            .into_iter()
            .take(8)
        {
            if is_triangle(&cell)? {
                return Ok(Some(cell));
            }
            for child in surface_read::elements(&cell, "AXChildren", deadline)?
                .into_iter()
                .take(8)
            {
                if is_triangle(&child)? {
                    return Ok(Some(child));
                }
            }
        }
        Ok(None)
    }

    /// The triangle answers -25205 to a press that expands the row (M-W0-8,
    /// 5/5), so the row's own state decides delivery, not the return code.
    fn press_triangle(
        row: &AXElement,
        triangle: &AXElement,
        expanded: bool,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        use core_foundation::{base::TCFType, string::CFString};
        prepare(triangle, deadline)?;
        let action = CFString::new("AXPress");
        let error = crate::tree::properties::ax_ipc::perform_action(
            triangle,
            action.as_concrete_TypeRef(),
            deadline,
        )?;
        triangle_outcome(
            crate::actions::ax_mutation::classify_perform("AXPress", error),
            || verify_disclosure(row, expanded, deadline),
            error,
        )
    }

    fn triangle_outcome(
        signal: Result<crate::actions::ax_mutation::PerformSignal, AdapterError>,
        verify: impl FnOnce() -> Result<DeliveryOutcome, AdapterError>,
        error: i32,
    ) -> Result<DeliveryOutcome, AdapterError> {
        use crate::actions::ax_mutation::PerformSignal;
        let signal = match signal {
            Ok(PerformSignal::ReportedUnsupported) => return Ok(DeliveryOutcome::NotDelivered),
            Ok(signal) => signal,
            Err(failure) => return Err(failure),
        };
        match verify() {
            Ok(DeliveryOutcome::DeliveredVerified) => Ok(DeliveryOutcome::DeliveredVerified),
            outcome if signal == PerformSignal::ReportedDelivered => {
                outcome.map_err(after_delivery)
            }
            _ => crate::actions::ax_mutation::settle_uninformative_perform("AXPress", error, false),
        }
    }

    fn stop_if_delivered(outcome: DeliveryOutcome) -> Option<DeliveryOutcome> {
        outcome.was_delivered().then_some(outcome)
    }

    fn disclosure_plan(current: Option<bool>, desired: bool) -> (bool, bool) {
        (current == Some(desired), current == Some(!desired))
    }

    fn verify_disclosure(
        element: &AXElement,
        expanded: bool,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let local_end = Instant::now() + Duration::from_millis(200);
        loop {
            let current_state = disclosed_state(element, deadline)?;
            match current_state {
                Some(current) if current == expanded => {
                    return Ok(DeliveryOutcome::DeliveredVerified);
                }
                _ => {}
            }
            if deadline.is_expired() {
                return Err(deadline.timeout_error().with_details(serde_json::json!({
                    "verification": "expanded_state_not_observed",
                })));
            }
            if std::time::Instant::now() >= local_end {
                return unobserved_delivery(current_state, expanded);
            }
            let pause = deadline.remaining_slice(Duration::from_millis(20))?;
            std::thread::sleep(pause.min(Duration::from_millis(20)));
        }
    }

    fn unobserved_delivery(
        last_state: Option<bool>,
        expanded: bool,
    ) -> Result<DeliveryOutcome, AdapterError> {
        if last_state == Some(!expanded) {
            Err(AdapterError::new(
                agent_desktop_core::ErrorCode::ActionFailed,
                "Disclosure action completed but the requested expanded state was not observed",
            )
            .with_disposition(agent_desktop_core::DeliverySemantics::delivered_unverified())
            .with_suggestion("Inspect the expanded state before deciding whether to retry."))
        } else {
            Ok(DeliveryOutcome::DeliveredUnverified)
        }
    }

    fn disclosed_state(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<Option<bool>, AdapterError> {
        let instant = deadline;
        if let Some(value) = crate::tree::surface_read::boolean(element, "AXExpanded", instant)? {
            return Ok(Some(value));
        }
        if let Some(value) = crate::tree::surface_read::boolean(element, "AXDisclosing", instant)? {
            return Ok(Some(value));
        }
        let role = crate::tree::surface_read::string(element, "AXRole", instant)?;
        if role.as_deref() != Some("AXDisclosureTriangle") {
            return Ok(None);
        }
        let value = crate::tree::properties::attributes::copy_value_typed_result(element, deadline)
            .map_err(|error| {
                crate::tree::query::read_error::semantic_read(error, "disclosure.value")
            })?;
        Ok(crate::tree::state_reader::expanded_state(
            role.as_deref(),
            None,
            value.as_deref(),
        ))
    }

    fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
        crate::tree::properties::attributes::set_messaging_timeout(element, deadline)
    }

    fn after_delivery(error: AdapterError) -> AdapterError {
        let mut delivery = crate::actions::DeliveryTracker::default();
        delivery.mark_delivered();
        delivery.annotate(error)
    }

    #[cfg(test)]
    mod tests {
        use super::{
            DeliveryOutcome, disclosure_plan, stop_if_delivered, triangle_outcome,
            unobserved_delivery,
        };
        use crate::actions::ax_mutation::PerformSignal;

        #[test]
        fn a_row_expanded_through_its_triangle_is_verified_by_the_row_whatever_the_code() {
            let verified = || Ok(DeliveryOutcome::DeliveredVerified);
            let unchanged = || Ok(DeliveryOutcome::DeliveredUnverified);
            assert_eq!(
                triangle_outcome(Ok(PerformSignal::Uninformative), verified, -25205).unwrap(),
                DeliveryOutcome::DeliveredVerified
            );
            assert_eq!(
                triangle_outcome(Ok(PerformSignal::ReportedUnsupported), verified, -25206).unwrap(),
                DeliveryOutcome::NotDelivered
            );
            assert_eq!(
                triangle_outcome(Ok(PerformSignal::Uninformative), unchanged, -25205)
                    .unwrap_err()
                    .disposition,
                agent_desktop_core::DeliverySemantics::uncertain()
            );
        }

        #[test]
        fn disclosure_plan_never_blindly_toggles_unknown_state() {
            assert_eq!(disclosure_plan(Some(true), true), (true, false));
            assert_eq!(disclosure_plan(Some(false), true), (false, true));
            assert_eq!(disclosure_plan(None, true), (false, false));
            assert_eq!(disclosure_plan(Some(false), false), (true, false));
            assert_eq!(disclosure_plan(Some(true), false), (false, true));
            assert_eq!(disclosure_plan(None, false), (false, false));
        }

        #[test]
        fn unobserved_delivery_preserves_honest_unknown_state() {
            assert_eq!(
                unobserved_delivery(Some(false), true)
                    .expect_err("known opposite state")
                    .disposition,
                agent_desktop_core::DeliverySemantics::delivered_unverified()
            );
            assert_eq!(
                unobserved_delivery(None, true).expect("unreadable state after delivery"),
                DeliveryOutcome::DeliveredUnverified
            );
            assert_eq!(
                stop_if_delivered(
                    unobserved_delivery(None, true).expect("unreadable state after delivery")
                ),
                Some(DeliveryOutcome::DeliveredUnverified)
            );
        }

        #[test]
        fn delivered_outcomes_halt_further_mutation_strategies() {
            assert_eq!(
                stop_if_delivered(DeliveryOutcome::DeliveredUnverified),
                Some(DeliveryOutcome::DeliveredUnverified)
            );
            assert_eq!(
                stop_if_delivered(DeliveryOutcome::DeliveredVerified),
                Some(DeliveryOutcome::DeliveredVerified)
            );
            assert_eq!(stop_if_delivered(DeliveryOutcome::NotDelivered), None);
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::{press_to_collapse, press_to_expand};
