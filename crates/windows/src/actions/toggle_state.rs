//! Toggle, Check and Uncheck.
//!
//! The toggle state is read first, and Check or Uncheck on a control already
//! in the target state delivers nothing. Otherwise the LegacyIAccessible
//! default action flips the control with no foreground change on Win32,
//! WinForms and WPF (W-W0-7, C3). TogglePattern flips it too but activates
//! the window, so it is the headed rung. Each flip is verified by re-reading
//! the state.

use agent_desktop_core::{ActionStep, AdapterError, Deadline, InteractionPolicy};
use std::time::Duration;

use crate::actions::chain::{after_delivery, poll_until};

use crate::tree::element::UIAElement;
use agent_desktop_core::DeliveryOutcome;

pub(crate) const TOGGLE_LABEL: &str = "TogglePattern.Toggle";

const VERIFY_TIMEOUT: Duration = Duration::from_millis(600);

const TOGGLE_SUGGESTION: &str =
    "Refresh the snapshot and inspect the checked state before deciding whether to retry.";

/// UIA `ToggleState` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToggleKind {
    Off,
    On,
    Indeterminate,
}

impl ToggleKind {
    pub(crate) fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Off),
            1 => Some(Self::On),
            2 => Some(Self::Indeterminate),
            _ => None,
        }
    }

    fn matches_checked(self, want_checked: bool) -> bool {
        matches!((self, want_checked), (Self::On, true) | (Self::Off, false))
    }
}

/// What the command asks for: any change, or one checked state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToggleGoal {
    Flip,
    Checked(bool),
}

impl ToggleGoal {
    fn satisfied_by(self, state: Option<ToggleKind>) -> bool {
        matches!(self, Self::Checked(want) if state.is_some_and(|state| state.matches_checked(want)))
    }
}

/// Flips toward `goal` and judges the flip from state re-reads.
///
/// A tri-state control needs two flips to reach checked from indeterminate.
/// The second flip is sent only after the first was seen to move the state:
/// flipping again when nothing moved could end back where it started while
/// the step reported delivery. Without a readable starting state a flip is
/// verified only when a re-read shows the requested checked state, and an
/// error after a flip keeps the evidence that the control was mutated.
pub(crate) fn drive_toggle(
    goal: ToggleGoal,
    deadline: Deadline,
    read: &mut dyn FnMut() -> Option<ToggleKind>,
    deliver: &mut dyn FnMut() -> Result<bool, AdapterError>,
) -> Result<DeliveryOutcome, AdapterError> {
    let Some(mut before) = read() else {
        if !deliver()? {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        let reached = poll_until(deadline, VERIFY_TIMEOUT, || goal.satisfied_by(read()))
            .map_err(after_delivery)?;
        return Ok(DeliveryOutcome::from_delivery(true, reached));
    };
    for attempt in 0..2 {
        let sent = if attempt == 0 {
            deliver()?
        } else {
            deliver().map_err(after_delivery)?
        };
        if !sent {
            return Ok(DeliveryOutcome::from_delivery(attempt > 0, false));
        }
        let mut now = None;
        let moved = poll_until(deadline, VERIFY_TIMEOUT, || {
            now = read();
            now.is_some_and(|state| state != before)
        })
        .map_err(after_delivery)?;
        if !moved {
            return Ok(DeliveryOutcome::observed(now.map(|_| false)));
        }
        if goal == ToggleGoal::Flip || goal.satisfied_by(now) {
            return Ok(DeliveryOutcome::Delivered(
                agent_desktop_core::DeliveryObservation::Verified,
            ));
        }
        if let Some(state) = now {
            before = state;
        }
    }
    Ok(DeliveryOutcome::Delivered(
        agent_desktop_core::DeliveryObservation::Unverified,
    ))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        ActionStep, AdapterError, Deadline, InteractionPolicy, TOGGLE_LABEL, TOGGLE_SUGGESTION,
        ToggleGoal, ToggleKind, UIAElement, drive_toggle,
    };
    use crate::actions::chain::{
        ALREADY_LABEL, LEGACY_LABEL, legacy_default_action, legacy_default_action_available,
        offered, rung,
    };
    use crate::actions::mutation::{classify_success, classify_write};
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_one;
    use agent_desktop_core::DeliveryOutcome;
    use uiautomation::patterns::UITogglePattern;

    pub(crate) fn toggle_steps(
        element: &UIAElement,
        goal: ToggleGoal,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        deadline.ensure_remaining()?;
        let toggle_ok = read_one(element, TreeProperty::ToggleAvailable).flag() == Some(true);
        if goal.satisfied_by(read_toggle_state(element)) {
            return Ok(vec![DeliveryOutcome::Satisfied.step(ALREADY_LABEL)]);
        }
        let mut read = || read_toggle_state(element);
        let mut legacy = || {
            drive_toggle(goal, deadline, &mut read, &mut || {
                legacy_default_action(element)
            })
        };
        let mut read = || read_toggle_state(element);
        let mut pattern =
            || drive_toggle(goal, deadline, &mut read, &mut || pattern_toggle(element));
        agent_desktop_core::execute_chain(
            &mut offered([
                rung(TOGGLE_LABEL, toggle_ok, true, &mut pattern),
                rung(
                    LEGACY_LABEL,
                    toggle_ok && legacy_default_action_available(element),
                    false,
                    &mut legacy,
                ),
            ]),
            policy,
            deadline,
            TOGGLE_SUGGESTION,
        )
    }

    fn read_toggle_state(element: &UIAElement) -> Option<ToggleKind> {
        read_one(element, TreeProperty::ToggleState)
            .number()
            .and_then(ToggleKind::from_i32)
    }

    fn pattern_toggle(element: &UIAElement) -> Result<bool, AdapterError> {
        match element.0.get_pattern::<UITogglePattern>() {
            Ok(pattern) => match pattern.toggle() {
                Ok(()) => classify_success(),
                Err(error) => classify_write("Toggle", TOGGLE_LABEL, &error),
            },
            Err(error) => classify_write("get_pattern", TOGGLE_LABEL, &error),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionStep, AdapterError, Deadline, InteractionPolicy, ToggleGoal, UIAElement};

    pub(crate) fn toggle_steps(
        _: &UIAElement,
        _: ToggleGoal,
        _: InteractionPolicy,
        _: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Toggle"))
    }
}

pub(crate) use imp::toggle_steps;

#[cfg(test)]
#[path = "toggle_state_tests.rs"]
mod tests;
