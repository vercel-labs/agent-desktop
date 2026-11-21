//! Rung engine for Windows element actions.
//!
//! Each rung names one mechanism, whether the live element offers it, and
//! whether it moves the desktop foreground. Every UI Automation control
//! pattern call (Invoke, Toggle, Value, RangeValue, ExpandCollapse,
//! SelectionItem, ScrollItem on proxied controls) activates the target's
//! window on every provider measured (W-W0-6, W-W0-7), so those rungs run
//! only under a headed policy. The LegacyIAccessible and Win32 message rungs
//! change no foreground state and run in both modes, always first.
//!
//! The engine runs the offered rungs in order and stops at the first that
//! delivers or finds the target already satisfied. A genuine `Err` aborts
//! the chain. When nothing delivered and a rung was held back only for its
//! foreground effect, the result is `POLICY_DENIED`, never a generic
//! exhaustion, so the caller learns that `--headed` would act.

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    StepMechanism,
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

/// Per-rung delivery judgment recorded into an [`ActionStep`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeliveryOutcome {
    NotDelivered,
    SatisfiedNoDelivery,
    DeliveredUnverified,
    DeliveredVerified,
    /// Delivered, but verification was withheld (secure / unreadable IsPassword).
    DeliveredUnobserved,
}

impl DeliveryOutcome {
    pub(crate) fn from_delivery(delivered: bool, verified: bool) -> Self {
        match (delivered, verified) {
            (false, _) => Self::NotDelivered,
            (true, false) => Self::DeliveredUnverified,
            (true, true) => Self::DeliveredVerified,
        }
    }

    /// Maps a delivered write's observation onto a chain outcome.
    ///
    /// `None` is the secure-field shape: the write landed and no value was
    /// re-read, so the step must not claim `verified: true` or `false`.
    pub(crate) fn from_observation(verified: Option<bool>) -> Self {
        match verified {
            Some(true) => Self::DeliveredVerified,
            Some(false) => Self::DeliveredUnverified,
            None => Self::DeliveredUnobserved,
        }
    }

    fn terminates_chain(self) -> bool {
        !matches!(self, Self::NotDelivered)
    }
}

/// One mechanism the engine may run.
pub(crate) struct ChainRung<'a> {
    pub(crate) label: &'static str,
    /// The live element offers this mechanism.
    pub(crate) available: bool,
    /// The mechanism moves the desktop foreground, so headless must not run it.
    pub(crate) requires_headed: bool,
    pub(crate) run: &'a mut dyn FnMut() -> Result<DeliveryOutcome, AdapterError>,
}

/// Runs the offered rungs in order, recording a step for every rung run.
pub(crate) fn execute_chain(
    deadline: Deadline,
    suggestion: &'static str,
    policy: InteractionPolicy,
    rungs: &mut [ChainRung<'_>],
) -> Result<Vec<ActionStep>, AdapterError> {
    let mut steps = Vec::new();
    let mut withheld = Vec::new();
    for rung in rungs.iter_mut().filter(|rung| rung.available) {
        deadline
            .ensure_remaining()
            .map_err(|expiry| expiry.with_disposition(DeliverySemantics::not_delivered()))?;
        if rung.requires_headed && !policy.is_headed() {
            withheld.push(rung.label);
            continue;
        }
        let outcome = (rung.run)()?;
        steps.push(build_step(rung.label, outcome));
        if outcome.terminates_chain() {
            return Ok(steps);
        }
    }
    if !withheld.is_empty() {
        return Err(foreground_denied(&withheld, policy));
    }
    Err(
        AdapterError::new(ErrorCode::ActionFailed, "All chain steps exhausted")
            .with_disposition(DeliverySemantics::not_delivered())
            .with_suggestion(suggestion),
    )
}

/// The refusal for an action whose only offered mechanisms move the foreground.
pub(crate) fn foreground_denied(
    labels: &[&'static str],
    policy: InteractionPolicy,
) -> AdapterError {
    AdapterError::policy_denied_for_policy(
        format!(
            "{} moves the desktop foreground window on Windows",
            labels.join(" and ")
        ),
        policy,
    )
    .with_details(serde_json::json!({
        "foreground_effect": true,
        "mechanisms": labels,
    }))
}

/// Re-dispositions an error raised after a rung already mutated the control.
/// The caller has positive evidence of delivery, so the retry permission the
/// inner error carried no longer describes the world the caller is in.
pub(crate) fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

pub(crate) fn build_step(label: &'static str, outcome: DeliveryOutcome) -> ActionStep {
    let built = match outcome {
        DeliveryOutcome::NotDelivered => ActionStep::skipped(label),
        DeliveryOutcome::SatisfiedNoDelivery => ActionStep::skipped(label).with_verified(true),
        DeliveryOutcome::DeliveredUnobserved => ActionStep::succeeded(label),
        DeliveryOutcome::DeliveredUnverified => ActionStep::succeeded(label).with_verified(false),
        DeliveryOutcome::DeliveredVerified => ActionStep::succeeded(label).with_verified(true),
    };
    built.with_mechanism(StepMechanism::SemanticApi)
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
