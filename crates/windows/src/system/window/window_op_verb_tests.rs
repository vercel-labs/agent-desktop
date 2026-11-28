use super::*;

#[test]
fn a_headless_restore_un_minimizes_without_activating() {
    assert_eq!(restore_command(InteractionPolicy::headless()), 4);
    assert_eq!(restore_command(InteractionPolicy::focus_fallback()), 9);
    assert_eq!(restore_command(InteractionPolicy::headed()), 9);
}

#[test]
fn a_headless_restore_of_a_minimized_window_is_refused_and_a_maximized_one_is_not() {
    let headless = InteractionPolicy::headless();
    let error = restore_permitted(true, headless).unwrap_err();
    assert_eq!(error.code, agent_desktop_core::ErrorCode::PolicyDenied);
    assert!(error.suggestion.unwrap().contains("--headed"));
    assert!(restore_permitted(false, headless).is_ok());
    assert!(restore_permitted(true, InteractionPolicy::headed()).is_ok());
}
