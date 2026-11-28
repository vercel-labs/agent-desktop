use super::{is_native_tab_class, selection_outcome, tab_index};
use agent_desktop_core::{DeliveryObservation, DeliveryOutcome};

#[test]
fn a_user32_tab_control_and_its_superclasses_are_native_tab_controls() {
    assert!(is_native_tab_class("SysTabControl32"));
    assert!(is_native_tab_class(
        "WindowsForms10.SysTabControl32.app.0.34f5582_r8_ad1"
    ));
    assert!(!is_native_tab_class(
        "WindowsForms10.BUTTON.app.0.34f5582_r8_ad1"
    ));
    assert!(!is_native_tab_class("Chrome_WidgetWin_1"));
}

#[test]
fn the_index_is_the_position_only_when_the_control_agrees_with_the_tree() {
    assert_eq!(tab_index(2, 5, 5), Some(2));
    assert_eq!(tab_index(0, 1, 1), Some(0));
    assert_eq!(tab_index(2, 5, 4), None);
    assert_eq!(tab_index(5, 5, 5), None);
    assert_eq!(tab_index(0, 5, -1), None);
}

#[test]
fn a_selection_that_did_not_follow_the_focus_is_handed_back_to_the_caller() {
    assert_eq!(
        selection_outcome(3, 3),
        Some(DeliveryOutcome::Delivered(DeliveryObservation::Verified))
    );
    assert_eq!(selection_outcome(1, 3), None);
    assert_eq!(selection_outcome(-1, 0), None);
}
