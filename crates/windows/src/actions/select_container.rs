//! Finding a named item through UI Automation's own virtualization contract.
//!
//! A container that offers `ItemContainerPattern` answers
//! `FindItemByProperty(Name)` for an item it has not built yet, and the item's
//! `VirtualizedItemPattern.Realize` builds it. No scroll is driven from here,
//! so it reaches the items of a XAML combo box whose own `ScrollPattern` is
//! absent (the Settings "Country or region" combo, probe 2026-11-07), and it
//! decides only by the patterns the element exposes. Every call here is a
//! control pattern call, so the caller runs it only where the policy allows a
//! foreground effect. When the item carries `VirtualizedItemPattern`, `Realize`
//! returns before the item is usable, so it is looked up again after `Realize`
//! and waited for until it is on screen; the reference from before `Realize` is
//! never selected. An item without `VirtualizedItemPattern` is already built
//! and is used directly after waiting for it to appear on screen.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

use crate::actions::chain::after_delivery;
use crate::actions::mutation::classify_mutation;
use crate::tree::automation::{UiaFailure, uia_failure_error};
use crate::tree::element::UIAElement;

const REALIZE_OPERATION: &str = "Realize";
const REALIZE_LABEL: &str = "VirtualizedItemPattern.Realize";

/// Whether the item had to be built before it could be used.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Realization {
    /// The lookup returned a built item; nothing was sent to the application.
    AlreadyBuilt,
    /// `Realize` was sent, so every later failure is a delivered request with an unverified outcome.
    Sent,
}

/// The second-item uniqueness lookup's answer for the select: `Ok` carries
/// the element a second `FindItemByProperty` found, and only `Ok(None)` —
/// the lookup answering "no item" — proves the requested name unique. A
/// lookup that failed proves nothing, so it refuses `AMBIGUOUS_TARGET`
/// before any selection, the same refusal the tree walk gives a second
/// same-name item.
fn accept_unique_lookup<T>(
    lookup: Result<Option<T>, UiaFailure>,
) -> Result<Option<T>, AdapterError> {
    match lookup {
        Ok(found) => Ok(found),
        Err(failure) => Err(uniqueness_unproven(failure)),
    }
}

/// The refusal for a uniqueness lookup that could not answer, keeping the
/// failed lookup's platform detail for diagnosis.
fn uniqueness_unproven(failure: UiaFailure) -> AdapterError {
    let classified = uia_failure_error(failure, "confirm the requested name unique");
    let mut error = AdapterError::ambiguous_target(
        "The item container could not prove the requested name unique",
    )
    .with_details(serde_json::json!({
        "kind": "ambiguous_select_value",
    }));
    if let Some(detail) = classified.platform_detail {
        error = error.with_platform_detail(detail);
    }
    error
}

/// The verdict on one `FindItemByProperty` result: `Ok(Some)` an item,
/// `Ok(None)` only when the call answered S_OK with no element — the
/// lookup's own "no item" answer, the one answer that can prove a name
/// absent. Every other failure stays `Err`: `UIA_E_ELEMENTNOTAVAILABLE`
/// and the not-found sentinel can mean the lookup could not run, so they
/// must never be read as absence.
fn lookup_verdict<T>(found: Result<T, UiaFailure>) -> Result<Option<T>, UiaFailure> {
    match found {
        Ok(element) => Ok(Some(element)),
        Err(failure) if failure.is_exhaustion() => Ok(None),
        Err(failure) => Err(failure),
    }
}

/// A lookup that failed returns its own error, never absence. After a
/// `Realize` was sent it is a delivered mutation with an unverified outcome,
/// and so is a lookup that then finds no usable item.
fn lookup_answer<T>(
    lookup: Result<Option<T>, UiaFailure>,
    realization: Realization,
) -> Result<Option<T>, AdapterError> {
    if realization == Realization::Sent && matches!(lookup, Ok(None)) {
        return Err(after_delivery(AdapterError::new(
            ErrorCode::ElementNotFound,
            "The item was realized but no selectable item answers the lookup after it",
        )));
    }
    lookup.map_err(|failure| {
        let error = uia_failure_error(failure, "look up the requested item");
        match realization {
            Realization::Sent => after_delivery(error),
            Realization::AlreadyBuilt => error.with_disposition(DeliverySemantics::not_delivered()),
        }
    })
}

/// The dispatched `Realize` call's verdict: `Ok` continues to the relookup
/// that verifies the realized item; `Err` is a delivered mutation whose
/// outcome is unverified — never an absent item and never a silent continue.
fn realize_verdict(outcome: Result<(), UiaFailure>) -> Result<(), AdapterError> {
    let Err(failure) = outcome else {
        return Ok(());
    };
    let error = match classify_mutation(REALIZE_OPERATION, REALIZE_LABEL, &failure) {
        Ok(_) => AdapterError::new(
            ErrorCode::ActionFailed,
            "Realize returned an empty result; whether the item was realized is unverified",
        ),
        Err(classified) => classified,
    };
    Err(after_delivery(error))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        AdapterError, Deadline, Realization, UIAElement, accept_unique_lookup, lookup_answer,
        lookup_verdict, realize_verdict,
    };
    use crate::actions::chain::poll_until;
    use crate::actions::select_search::{accept_selection_match, selection_item_available};
    use crate::tree::automation::{UiaFailure, automation_client, failure_of};
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use crate::tree::walk::walker_source::same_element;
    use std::time::Duration;
    use uiautomation::variants::Variant;
    use windows::Win32::UI::Accessibility::{
        IUIAutomationElement, IUIAutomationItemContainerPattern,
        IUIAutomationVirtualizedItemPattern, UIA_ItemContainerPatternId, UIA_NamePropertyId,
        UIA_VirtualizedItemPatternId,
    };

    const READY_TIMEOUT: Duration = Duration::from_millis(800);

    /// The realized item named exactly `value` in `container`, or `None` when
    /// the container offers no `ItemContainerPattern`, does not know the name,
    /// or cannot realize it. A second item with the same name — or a
    /// uniqueness lookup that fails — is `AMBIGUOUS_TARGET`, as in the tree
    /// walk; only a lookup that answers "no item" proves the name unique. A
    /// `Realize` that fails after dispatch is `delivered_unverified`, never
    /// an absent item.
    pub(crate) fn realize_named_item(
        container: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<Option<UIAElement>, AdapterError> {
        deadline.ensure_remaining()?;
        let raw: &IUIAutomationElement = container.0.as_ref();
        let Ok(pattern) = (unsafe {
            raw.GetCurrentPatternAs::<IUIAutomationItemContainerPattern>(UIA_ItemContainerPatternId)
        }) else {
            return Ok(None);
        };
        let name = Variant::from(value);
        let Some(first) =
            lookup_answer(find_after(&pattern, None, &name), Realization::AlreadyBuilt)?
        else {
            return Ok(None);
        };
        let first = UIAElement::from(uiautomation::UIElement::from(first));
        if let Some(second) =
            accept_unique_lookup(find_after(&pattern, Some(first.0.as_ref()), &name))?
        {
            let second = UIAElement::from(uiautomation::UIElement::from(second));
            if !same_element(&automation_client()?, &first, &second) {
                accept_selection_match(true)?;
            }
        }
        deadline.ensure_remaining()?;
        let virtualized = unsafe {
            first
                .0
                .as_ref()
                .GetCurrentPatternAs::<IUIAutomationVirtualizedItemPattern>(
                    UIA_VirtualizedItemPatternId,
                )
        };
        let (item, realization) = match virtualized {
            Ok(virtualized) => {
                realize_verdict(
                    unsafe { virtualized.Realize() }
                        .map_err(|error| failure_of(&uiautomation::Error::from(error))),
                )?;
                let Some(again) =
                    lookup_answer(find_after(&pattern, None, &name), Realization::Sent)?
                else {
                    return Ok(None);
                };
                (
                    UIAElement::from(uiautomation::UIElement::from(again)),
                    Realization::Sent,
                )
            }
            Err(_) => (first, Realization::AlreadyBuilt),
        };
        wait_until_on_screen(deadline, || {
            read_one(&item, TreeProperty::IsOffscreen).flag() == Some(false)
        })?;
        if selection_item_available(&item) {
            return Ok(Some(item));
        }
        lookup_answer(Ok(None), realization)
    }

    pub(crate) fn wait_until_on_screen(
        deadline: Deadline,
        on_screen: impl FnMut() -> bool,
    ) -> Result<(), AdapterError> {
        poll_until(deadline, READY_TIMEOUT, on_screen);
        deadline.ensure_remaining()
    }

    /// `FindItemByProperty` after `after`, judged by `lookup_verdict`:
    /// `Ok(None)` is only the call's own "no item" answer; every failure,
    /// `UIA_E_ELEMENTNOTAVAILABLE` included, stays `Err`.
    fn find_after(
        pattern: &IUIAutomationItemContainerPattern,
        after: Option<&IUIAutomationElement>,
        name: &Variant,
    ) -> Result<Option<IUIAutomationElement>, UiaFailure> {
        let found = unsafe { pattern.FindItemByProperty(after, UIA_NamePropertyId, name.as_ref()) }
            .map_err(|error| failure_of(&uiautomation::Error::from(error)));
        lookup_verdict(found)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, Deadline, UIAElement};

    pub(crate) fn realize_named_item(
        _container: &UIAElement,
        _value: &str,
        _deadline: Deadline,
    ) -> Result<Option<UIAElement>, AdapterError> {
        Ok(None)
    }
}

pub(crate) use imp::realize_named_item;

#[cfg(test)]
mod tests {
    use super::{
        Realization, accept_unique_lookup, lookup_answer, lookup_verdict, realize_verdict,
    };
    use crate::system::runtime::hresult::{E_FAIL, UIA_E_ELEMENTNOTAVAILABLE, UIA_E_TIMEOUT};
    use crate::tree::automation::{ERR_NONE, ERR_NOTFOUND, ERR_TIMEOUT, UiaFailure};
    use agent_desktop_core::{DeliverySemantics, ErrorCode, RetryDisposition};

    #[test]
    fn a_failed_uniqueness_lookup_refuses_before_any_selection() {
        for failure in [
            UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE),
            UiaFailure::Hresult(UIA_E_TIMEOUT),
            UiaFailure::Hresult(E_FAIL),
            UiaFailure::Sentinel(ERR_TIMEOUT),
        ] {
            let error = accept_unique_lookup::<()>(Err(failure)).expect_err("unproven");
            assert_eq!(error.code, ErrorCode::AmbiguousTarget);
            assert_eq!(error.disposition, DeliverySemantics::not_delivered());
            assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
        }
    }

    #[test]
    fn only_a_lookup_that_answered_no_item_proves_the_name_unique() {
        assert!(
            accept_unique_lookup::<()>(Ok(None))
                .expect("unique")
                .is_none()
        );
        assert_eq!(accept_unique_lookup(Ok(Some(3))).expect("found"), Some(3));
    }

    #[test]
    fn only_a_succeeded_lookup_with_no_element_answers_absence() {
        assert_eq!(lookup_verdict(Ok(3)), Ok(Some(3)));
        assert_eq!(
            lookup_verdict::<()>(Err(UiaFailure::Sentinel(ERR_NONE))),
            Ok(None)
        );
        for failure in [
            UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE),
            UiaFailure::Sentinel(ERR_NOTFOUND),
            UiaFailure::Hresult(UIA_E_TIMEOUT),
            UiaFailure::Hresult(E_FAIL),
            UiaFailure::Sentinel(ERR_TIMEOUT),
        ] {
            assert_eq!(lookup_verdict::<()>(Err(failure)), Err(failure));
        }
    }

    #[test]
    fn a_uniqueness_lookup_that_cannot_run_refuses_before_any_selection() {
        for failure in [
            UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE),
            UiaFailure::Sentinel(ERR_NOTFOUND),
        ] {
            let error =
                accept_unique_lookup(lookup_verdict::<()>(Err(failure))).expect_err("unproven");
            assert_eq!(error.code, ErrorCode::AmbiguousTarget);
            assert_eq!(error.disposition, DeliverySemantics::not_delivered());
            assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
        }
    }

    #[test]
    fn a_failed_first_lookup_is_its_own_error_and_never_absence() {
        assert!(
            lookup_answer(Ok(Some(3)), Realization::AlreadyBuilt)
                .expect("found")
                .is_some()
        );
        assert!(
            lookup_answer::<()>(Ok(None), Realization::AlreadyBuilt)
                .expect("none")
                .is_none()
        );
        let error = lookup_answer::<()>(
            Err(UiaFailure::Hresult(UIA_E_TIMEOUT)),
            Realization::AlreadyBuilt,
        )
        .expect_err("failed lookup");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }

    #[test]
    fn a_relookup_after_realize_never_falls_back_to_the_item_from_before() {
        assert_eq!(
            lookup_answer(Ok(Some(3)), Realization::Sent).expect("found"),
            Some(3)
        );
        let empty = lookup_answer::<()>(Ok(None), Realization::Sent).expect_err("realize was sent");
        assert_eq!(empty.code, ErrorCode::ElementNotFound);
        assert_eq!(empty.disposition, DeliverySemantics::delivered_unverified());
        assert_eq!(empty.disposition.retry(), RetryDisposition::Unsafe);
        let error = lookup_answer::<()>(Err(UiaFailure::Hresult(UIA_E_TIMEOUT)), Realization::Sent)
            .expect_err("failed relookup");
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        let unsent = lookup_answer::<()>(
            Err(UiaFailure::Hresult(UIA_E_TIMEOUT)),
            Realization::AlreadyBuilt,
        )
        .expect_err("failed relookup");
        assert_eq!(unsent.disposition, DeliverySemantics::not_delivered());
    }

    #[test]
    fn an_item_the_lookup_returned_already_built_is_used_without_a_second_lookup_state() {
        assert_eq!(
            lookup_answer::<()>(Ok(None), Realization::AlreadyBuilt).expect("absent"),
            None
        );
        assert_eq!(
            lookup_answer(Ok(Some(3)), Realization::AlreadyBuilt).expect("present"),
            Some(3)
        );
        let error = lookup_answer::<()>(
            Err(UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE)),
            Realization::AlreadyBuilt,
        )
        .expect_err("not delivered");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }

    #[test]
    fn a_failed_realize_is_delivered_unverified_and_never_claims_absence() {
        assert!(realize_verdict(Ok(())).is_ok());
        for failure in [
            UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE),
            UiaFailure::Hresult(UIA_E_TIMEOUT),
            UiaFailure::Hresult(E_FAIL),
            UiaFailure::Sentinel(ERR_TIMEOUT),
            UiaFailure::Sentinel(ERR_NOTFOUND),
            UiaFailure::Sentinel(ERR_NONE),
        ] {
            let error = realize_verdict(Err(failure)).expect_err("delivered unverified");
            assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
            assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
            assert_ne!(error.code, ErrorCode::ElementNotFound);
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "select_container_realize_tests.rs"]
mod realize_tests;
