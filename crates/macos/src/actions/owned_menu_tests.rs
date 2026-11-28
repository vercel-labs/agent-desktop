use super::menu_visibility;

#[test]
fn menu_verification_requires_visible_nonempty_geometry() {
    let bounds = agent_desktop_core::Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };
    assert_eq!(menu_visibility(None, Some(false), Some(bounds)), Some(true));
    assert_eq!(menu_visibility(Some(true), None, Some(bounds)), Some(true));
    assert_eq!(menu_visibility(None, None, Some(bounds)), None);
    assert_eq!(menu_visibility(Some(true), None, None), None);
    assert_eq!(
        menu_visibility(Some(false), Some(false), Some(bounds)),
        Some(false)
    );
    assert_eq!(
        menu_visibility(Some(true), Some(true), Some(bounds)),
        Some(false)
    );
    assert_eq!(
        menu_visibility(
            Some(true),
            Some(false),
            Some(agent_desktop_core::Rect {
                width: 0.0,
                ..bounds
            })
        ),
        Some(false)
    );
}
