use super::*;
use agent_desktop_core::ErrorCode;

#[test]
fn headed_focus_shares_provenance_and_preserve_pointer_checks_for_gestures() {
    use agent_desktop_core::{Action, ActionRequest, HeadedFocus};
    for action in [
        Action::Click,
        Action::RightClick,
        Action::DoubleClick,
        Action::TripleClick,
    ] {
        let request = ActionRequest::headed(action.clone());
        let native = headed_focus(&request, || Ok(true)).unwrap().unwrap();
        assert_eq!(
            native,
            if matches!(action, Action::Click) {
                HeadedFocus::Application
            } else {
                HeadedFocus::ApplicationAndCursor
            }
        );
        assert_eq!(native.uses_cursor(), !matches!(action, Action::Click));
        assert_eq!(
            headed_focus(&request, || Ok(false)).unwrap(),
            Some(HeadedFocus::WindowAndCursor)
        );
        let error = headed_focus(&request, || {
            Err(AdapterError::stale_ref_because("identity changed"))
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleRef);
        let headless = ActionRequest::headless(action);
        assert_eq!(
            headed_focus(&headless, || panic!(
                "headless requirement must not resolve"
            ))
            .unwrap(),
            None
        );
    }
}

#[test]
fn non_pointer_headed_actions_do_not_probe_menu_ancestry() {
    let request = agent_desktop_core::ActionRequest::headed(agent_desktop_core::Action::SetFocus);
    assert_eq!(
        headed_focus(&request, || panic!("not a menu gesture")).unwrap(),
        Some(agent_desktop_core::HeadedFocus::Window)
    );
}
