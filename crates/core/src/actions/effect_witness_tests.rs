use super::{EffectWitness, verdict};
use crate::actions::focus_witness::FocusWitness;
use crate::actions::{observed_effect::ObservedEffect, witness_change::WitnessChange};

fn read() -> EffectWitness {
    let mut witness = EffectWitness::default();
    witness.target.selected = Some(false);
    witness.target.value = Some("0".into());
    witness.target.surfaces = Some(0);
    witness.focus.element = Some("list".into());
    witness.focus.inside_target = Some(false);
    witness.focus.in_target_window = Some(true);
    witness.focus.value = Some("a".into());
    witness.window.title = Some("Downloads".into());
    witness.window.content = Some(7);
    witness.window.windows = Some(1);
    witness.window.present = Some(true);
    witness
}

#[test]
fn no_change_is_not_observed() {
    let before = read();
    assert!(read().changes_since(&before).is_empty());
    assert_eq!(verdict(&[]), ObservedEffect::NotObserved);
}

#[test]
fn a_change_on_or_under_the_target_is_an_observed_effect() {
    let before = read();
    for (change, expected) in [
        (
            (|w: &mut EffectWitness| w.target.selected = Some(true)) as fn(&mut EffectWitness),
            WitnessChange::TargetSelected,
        ),
        (
            |w| w.target.surfaces = Some(1),
            WitnessChange::SurfaceOpened,
        ),
        (
            |w| w.window.present = Some(false),
            WitnessChange::WindowClosed,
        ),
        (
            |w| {
                w.focus.element = Some("target".into());
                w.focus.inside_target = Some(true);
            },
            WitnessChange::FocusIntoTarget,
        ),
    ] {
        let mut after = read();
        change(&mut after);
        let changes = after.changes_since(&before);
        assert_eq!(changes, [expected]);
        assert_eq!(verdict(&changes), ObservedEffect::Observed);
    }
}

#[test]
fn a_change_the_application_can_make_by_itself_is_indeterminate() {
    let before = read();
    for (change, expected) in [
        (
            (|w: &mut EffectWitness| w.window.title = Some("Recents".into()))
                as fn(&mut EffectWitness),
            WitnessChange::WindowTitle,
        ),
        (|w| w.window.content = Some(8), WitnessChange::WindowContent),
        (
            |w| w.target.value = Some("00:02".into()),
            WitnessChange::TargetValue,
        ),
        (|w| w.window.windows = Some(2), WitnessChange::WindowCount),
        (
            |w| w.focus.element = Some("field".into()),
            WitnessChange::FocusMovedInWindow,
        ),
        (
            |w| w.focus.value = Some("b".into()),
            WitnessChange::FocusedValue,
        ),
    ] {
        let mut after = read();
        change(&mut after);
        let changes = after.changes_since(&before);
        assert_eq!(changes, [expected]);
        assert_eq!(verdict(&changes), ObservedEffect::Indeterminate);
    }
}

#[test]
fn a_field_read_only_once_and_a_state_that_goes_away_are_no_evidence() {
    let before = read();
    let mut after = read();
    after.window.title = None;
    after.focus.element = None;
    assert!(after.changes_since(&before).is_empty());

    let mut selected = read();
    selected.target.selected = Some(true);
    selected.target.surfaces = Some(1);
    let mut gone = selected.clone();
    gone.target.selected = Some(false);
    gone.target.surfaces = Some(0);
    assert!(gone.changes_since(&selected).is_empty());
}

#[test]
fn focus_that_arrives_in_the_target_from_no_focus_is_a_change() {
    let mut before = read();
    before.focus = FocusWitness::nothing_focused();
    let mut now = read();
    now.focus.element = Some("target".into());
    now.focus.inside_target = Some(true);

    assert!(
        now.changes_since(&before)
            .contains(&WitnessChange::FocusIntoTarget)
    );
}
