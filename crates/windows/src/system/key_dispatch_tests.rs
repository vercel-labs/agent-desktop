use agent_desktop_core::InteractionPolicy;

#[test]
fn key_dispatch_never_calls_activation_apis() {
    let src = include_str!("key_dispatch.rs");
    assert!(
        !src.contains("focus_window"),
        "verify-only path must not call focus_window"
    );
    assert!(
        !src.contains("window_activate"),
        "verify-only path must not call window_activate"
    );
    assert!(
        !src.contains("SetForegroundWindow"),
        "verify-only path must not raise via SetForegroundWindow"
    );
}

#[test]
fn all_three_interaction_policies_are_constructible_and_distinct() {
    let headless = InteractionPolicy::headless();
    let focus_fallback = InteractionPolicy::focus_fallback();
    let headed = InteractionPolicy::headed();
    assert!(!headless.allow_focus_steal);
    assert!(!headless.allow_cursor_move);
    assert!(focus_fallback.allow_focus_steal);
    assert!(!focus_fallback.allow_cursor_move);
    assert!(!focus_fallback.is_headed());
    assert!(headed.allow_focus_steal);
    assert!(headed.allow_cursor_move);
    assert!(headed.is_headed());
}
