//! Windows rungs for the core delivery chain (`agent_desktop_core::execute_chain`).
//!
//! Each rung names one mechanism, whether the live element offers it, and
//! whether it takes the desktop foreground. Every UI Automation control
//! pattern call (Invoke, Toggle, Value, RangeValue, ExpandCollapse,
//! SelectionItem, ScrollItem on proxied controls) activates the target's
//! window on every provider measured (W-W0-6, W-W0-7), so those rungs declare
//! that effect and core runs them only when the policy allows it. The
//! LegacyIAccessible and Win32 message rungs change no foreground state and
//! run first in both modes. Core stops at the first rung that delivers or
//! finds the target already satisfied, and refuses with `POLICY_DENIED` when
//! nothing was delivered and a rung was held back for its effect.

use agent_desktop_core::{
    AdapterError, ChainRung, Deadline, DeliveryOutcome, DeliverySemantics, Effect,
};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use crate::actions::mutation::{classify_success, classify_write};
use crate::system::runtime::hresult::UIA_E_ELEMENTNOTENABLED;
#[cfg(target_os = "windows")]
use crate::tree::automation::{UiaFailure, failure_of};
#[cfg(target_os = "windows")]
use crate::tree::element::UIAElement;
#[cfg(target_os = "windows")]
use crate::tree::properties::property_ids::TreeProperty;
#[cfg(target_os = "windows")]
use crate::tree::properties::read_one;
#[cfg(target_os = "windows")]
use uiautomation::patterns::{UIInvokePattern, UILegacyIAccessiblePattern};

pub(crate) const INVOKE_LABEL: &str = "InvokePattern.Invoke";
pub(crate) const LEGACY_LABEL: &str = "LegacyIAccessible.DoDefaultAction";
pub(crate) const ALREADY_LABEL: &str = "AlreadyInState";

const POLL_SLICE: Duration = Duration::from_millis(25);
const SELFLAG_TAKESELECTION: i32 = 0x2;

/// Caps a verification poll window by the remaining action budget.
pub(crate) fn capped_verification_end(
    deadline: Deadline,
    cap: Duration,
) -> Result<Instant, AdapterError> {
    let remaining = deadline.remaining();
    if remaining.is_zero() {
        return Err(deadline.timeout_error());
    }
    let local = Instant::now() + cap;
    Ok(Instant::now()
        .checked_add(remaining)
        .map_or(local, |end| end.min(local)))
}

/// Re-reads until `reached` holds or `cap`, bounded by the deadline, passes.
pub(crate) fn poll_until(
    deadline: Deadline,
    cap: Duration,
    mut reached: impl FnMut() -> bool,
) -> Result<bool, AdapterError> {
    let end = capped_verification_end(deadline, cap)?;
    loop {
        if reached() {
            return Ok(true);
        }
        if Instant::now() >= end {
            return Ok(false);
        }
        std::thread::sleep(deadline.remaining_slice(POLL_SLICE)?);
    }
}

/// What a UI Automation control pattern call does to the desktop: it
/// activates the target's window on every provider measured (W-W0-6,
/// W-W0-7). The LegacyIAccessible and Win32 message rungs declare nothing.
const TAKES_FOREGROUND: &[Effect] = &[Effect::FocusSteal];

/// One rung of a Windows chain, or none when the live element does not offer
/// the mechanism, so a mechanism the element lacks records no step.
pub(crate) fn rung<'a>(
    label: &'static str,
    available: bool,
    takes_foreground: bool,
    run: &'a mut dyn FnMut() -> Result<DeliveryOutcome, AdapterError>,
) -> Option<ChainRung<'a>> {
    available.then(|| ChainRung {
        label,
        effects: if takes_foreground {
            TAKES_FOREGROUND
        } else {
            &[]
        },
        run: Box::new(run),
    })
}

/// The rungs the element offers, in order, for `agent_desktop_core::execute_chain`.
pub(crate) fn offered<const N: usize>(rungs: [Option<ChainRung<'_>>; N]) -> Vec<ChainRung<'_>> {
    rungs.into_iter().flatten().collect()
}

/// Re-dispositions an error raised after a rung already mutated the control.
/// The caller has positive evidence of delivery, so the retry permission the
/// inner error carried no longer describes the world the caller is in.
pub(crate) fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

/// Whether `InvokePattern` is advertised.
#[cfg(target_os = "windows")]
pub(crate) fn invoke_available(element: &UIAElement) -> bool {
    read_one(element, TreeProperty::InvokeAvailable).flag() == Some(true)
}

/// Invokes `InvokePattern` and classifies the write.
#[cfg(target_os = "windows")]
pub(crate) fn invoke_pattern_delivered(element: &UIAElement) -> Result<bool, AdapterError> {
    match element.0.get_pattern::<UIInvokePattern>() {
        Ok(pattern) => match pattern.invoke() {
            Ok(()) => classify_success(),
            Err(error) => classify_write("Invoke", INVOKE_LABEL, &error),
        },
        Err(error) => classify_write("get_pattern", INVOKE_LABEL, &error),
    }
}

/// Whether the element names a LegacyIAccessible default action.
#[cfg(target_os = "windows")]
pub(crate) fn legacy_default_action_available(element: &UIAElement) -> bool {
    read_one(element, TreeProperty::LegacyDefaultAction)
        .non_blank_text()
        .is_some()
}

/// Runs the LegacyIAccessible default action and classifies the write.
#[cfg(target_os = "windows")]
pub(crate) fn legacy_default_action(element: &UIAElement) -> Result<bool, AdapterError> {
    legacy_write(element, LEGACY_LABEL, |pattern| pattern.do_default_action())
}

/// Selects the element alone through LegacyIAccessible `Select` with
/// `SELFLAG_TAKESELECTION`, and classifies the write.
#[cfg(target_os = "windows")]
pub(crate) fn legacy_select(element: &UIAElement) -> Result<bool, AdapterError> {
    legacy_write(element, "LegacyIAccessible.Select", |pattern| {
        pattern.select(SELFLAG_TAKESELECTION)
    })
}

/// Writes the element's value through LegacyIAccessible `SetValue`.
#[cfg(target_os = "windows")]
pub(crate) fn legacy_set_value(element: &UIAElement, value: &str) -> Result<bool, AdapterError> {
    legacy_write(element, "LegacyIAccessible.SetValue", |pattern| {
        pattern.set_value(value)
    })
}

/// Whether a Legacy call's refusal means the route is absent.
///
/// The LegacyIAccessible bridge answers `UIA_E_ELEMENTNOTENABLED` both for a
/// disabled element and for an enabled element that has no such member: a
/// DirectUI row's "Double Click" default action and the DirectUI search
/// box's `SetValue` fail that way while the element reports IsEnabled true
/// (P3). On an enabled element the refusal is an absent route, so the chain
/// moves on; on a disabled element it stays a refusal.
pub(crate) fn legacy_refusal_is_absent(hresult: i32, element_enabled: Option<bool>) -> bool {
    hresult == UIA_E_ELEMENTNOTENABLED && element_enabled == Some(true)
}

#[cfg(target_os = "windows")]
fn legacy_write(
    element: &UIAElement,
    api: &'static str,
    call: impl FnOnce(&UILegacyIAccessiblePattern) -> uiautomation::Result<()>,
) -> Result<bool, AdapterError> {
    let operation = api.trim_start_matches("LegacyIAccessible.");
    let pattern = match element.0.get_pattern::<UILegacyIAccessiblePattern>() {
        Ok(pattern) => pattern,
        Err(error) => return classify_write("get_pattern", api, &error),
    };
    match call(&pattern) {
        Ok(()) => classify_success(),
        Err(error) => match failure_of(&error) {
            UiaFailure::Hresult(hresult)
                if legacy_refusal_is_absent(
                    hresult,
                    read_one(element, TreeProperty::IsEnabled).flag(),
                ) =>
            {
                Ok(false)
            }
            _ => classify_write(operation, api, &error),
        },
    }
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
