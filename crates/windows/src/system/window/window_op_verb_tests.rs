use super::*;

#[test]
fn a_headless_restore_un_minimizes_without_activating() {
    assert_eq!(restore_command(InteractionPolicy::headless()), 4);
    assert_eq!(restore_command(InteractionPolicy::focus_fallback()), 9);
    assert_eq!(restore_command(InteractionPolicy::headed()), 9);
}

#[test]
fn only_a_headless_restore_is_placed_below_the_foreground_window() {
    let headless = InteractionPolicy::headless();
    assert!(restore_goes_below_foreground(ShowVerb::Restore, headless));
    assert!(!restore_goes_below_foreground(
        ShowVerb::Restore,
        InteractionPolicy::headed()
    ));
    assert!(!restore_goes_below_foreground(ShowVerb::Minimize, headless));
    assert!(!restore_goes_below_foreground(ShowVerb::Maximize, headless));
}
