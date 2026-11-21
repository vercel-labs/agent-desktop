use serde::{Deserialize, Serialize};

use crate::actions::{
    focus_witness::FocusWitness, observed_effect::ObservedEffect, target_witness::TargetWitness,
    window_witness::WindowWitness, witness_change::WitnessChange,
};

/// What an adapter reads around an action whose effect lands outside the
/// target's own state: a click, a select. A field the platform could not read
/// is `None` and never counts as a change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectWitness {
    pub target: TargetWitness,
    pub focus: FocusWitness,
    pub window: WindowWitness,
}

impl EffectWitness {
    /// The changes from `before` to `self`. A change counts only when both
    /// reads have the field. A selection or a surface counts only when it
    /// appears, because one that goes away says nothing about this action.
    pub fn changes_since(&self, before: &Self) -> Vec<WitnessChange> {
        let (now, then) = (self, before);
        let focus_moved = differs(&then.focus.element, &now.focus.element);
        [
            (
                then.target.selected == Some(false) && now.target.selected == Some(true),
                WitnessChange::TargetSelected,
            ),
            (
                differs(&then.target.value, &now.target.value),
                WitnessChange::TargetValue,
            ),
            (
                grew(then.target.surfaces, now.target.surfaces),
                WitnessChange::SurfaceOpened,
            ),
            (
                focus_moved && now.focus.inside_target == Some(true),
                WitnessChange::FocusIntoTarget,
            ),
            (
                focus_moved
                    && now.focus.inside_target != Some(true)
                    && now.focus.in_target_window == Some(true),
                WitnessChange::FocusMovedInWindow,
            ),
            (
                !focus_moved
                    && now.focus.in_target_window == Some(true)
                    && differs(&then.focus.value, &now.focus.value),
                WitnessChange::FocusedValue,
            ),
            (
                then.window.present == Some(true) && now.window.present == Some(false),
                WitnessChange::WindowClosed,
            ),
            (
                differs(&then.window.title, &now.window.title),
                WitnessChange::WindowTitle,
            ),
            (
                differs(&then.window.content, &now.window.content),
                WitnessChange::WindowContent,
            ),
            (
                differs(&then.window.windows, &now.window.windows),
                WitnessChange::WindowCount,
            ),
        ]
        .into_iter()
        .filter_map(|(changed, change)| changed.then_some(change))
        .collect()
    }
}

/// The verdict of a list of changes: any strong change is an observed
/// effect, weak changes alone are indeterminate, and no change is not
/// observed.
pub(crate) fn verdict(changes: &[WitnessChange]) -> ObservedEffect {
    if changes.iter().any(|change| change.is_strong()) {
        ObservedEffect::Observed
    } else if changes.is_empty() {
        ObservedEffect::NotObserved
    } else {
        ObservedEffect::Indeterminate
    }
}

fn differs<T: PartialEq>(before: &Option<T>, after: &Option<T>) -> bool {
    matches!((before, after), (Some(before), Some(after)) if before != after)
}

fn grew(before: Option<u32>, after: Option<u32>) -> bool {
    matches!((before, after), (Some(before), Some(after)) if after > before)
}

#[cfg(test)]
#[path = "effect_witness_tests.rs"]
mod tests;
