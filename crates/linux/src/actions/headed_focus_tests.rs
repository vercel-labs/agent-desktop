use super::*;
use agent_desktop_core::Action;

#[test]
fn a_headed_action_focuses_nothing_on_wayland_and_keeps_its_focus_on_x11() {
    let set_value = ActionRequest::headed(Action::SetValue("x".into()));
    let click = ActionRequest::headed(Action::Click);
    assert_eq!(headed_focus_on(&DisplayServer::Wayland, &set_value), None);
    assert_eq!(headed_focus_on(&DisplayServer::Wayland, &click), None);
    let x11 = DisplayServer::X11(":0".into());
    assert_eq!(headed_focus_on(&x11, &set_value), Some(HeadedFocus::Window));
    assert_eq!(
        headed_focus_on(&x11, &click),
        Some(HeadedFocus::WindowAndCursor)
    );
}
