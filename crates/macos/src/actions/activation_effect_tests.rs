use super::imp::{press_delivered, press_reaction};
use agent_desktop_core::{DeliveryObservation, WitnessChange};

#[test]
fn a_press_is_delivered_by_a_change_in_the_target_or_its_focus_but_not_by_the_window_alone() {
    for change in [
        WitnessChange::TargetSelected,
        WitnessChange::TargetValue,
        WitnessChange::SurfaceOpened,
        WitnessChange::FocusIntoTarget,
        WitnessChange::FocusMovedInWindow,
        WitnessChange::FocusedValue,
    ] {
        assert!(press_delivered(&[change]), "{change:?}");
    }
    for change in [
        WitnessChange::WindowTitle,
        WitnessChange::WindowContent,
        WitnessChange::WindowCount,
        WitnessChange::TargetGone,
    ] {
        assert!(!press_delivered(&[change]), "{change:?}");
    }
    assert!(!press_delivered(&[]));
}

#[test]
fn only_a_strong_change_verifies_a_press_and_a_weak_one_only_shows_it_arrived() {
    for change in [
        WitnessChange::TargetValue,
        WitnessChange::FocusMovedInWindow,
        WitnessChange::FocusedValue,
    ] {
        assert_eq!(
            press_reaction(&[change]),
            Some(DeliveryObservation::Unverified),
            "{change:?}"
        );
    }
    for change in [
        WitnessChange::TargetSelected,
        WitnessChange::SurfaceOpened,
        WitnessChange::FocusIntoTarget,
    ] {
        assert_eq!(
            press_reaction(&[change]),
            Some(DeliveryObservation::Verified),
            "{change:?}"
        );
    }
    assert_eq!(press_reaction(&[WitnessChange::WindowTitle]), None);
}
