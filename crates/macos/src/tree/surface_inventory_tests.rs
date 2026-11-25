use super::*;
use agent_desktop_core::Deadline;

fn surface(id: &str, kind: &str) -> SurfaceInfo {
    SurfaceInfo {
        id: id.into(),
        kind: kind.into(),
        title: Some(format!("{kind} title")),
        item_count: Some(3),
        unclassified: Vec::new(),
    }
}

#[test]
fn shared_menu_discovery_preserves_other_surfaces_and_existing_menu_ids() {
    let window_surfaces = vec![
        surface("focused-window", "sheet"),
        surface("focused-window/children/2", "popover"),
        surface("focused-window/children/4", "alert"),
    ];
    for menus in [
        vec![],
        vec![surface("app/menubar/children/1/children/0", "menu")],
    ] {
        let mut expected = menus.clone();
        expected.extend(window_surfaces.clone());
        let actual = list_surfaces_with(
            Deadline::after(1000).unwrap(),
            || Ok(window_surfaces.clone()),
            || Ok(menus),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}

#[test]
fn incomplete_shared_discovery_is_not_reported_as_an_empty_inventory() {
    for failed_menu in [true, false] {
        let read = |fail| {
            if fail {
                Err(
                    AdapterError::new(agent_desktop_core::ErrorCode::AppUnresponsive, "incomplete")
                        .with_details(
                            serde_json::json!({ "complete": false, "attribute": "AXChildren" }),
                        ),
                )
            } else {
                Ok(vec![surface("observed", "sheet")])
            }
        };
        let error = list_surfaces_with(
            Deadline::after(1000).unwrap(),
            || read(!failed_menu),
            || read(failed_menu),
        )
        .unwrap_err();
        let details = error.details.unwrap();
        assert_eq!(details["complete"], false);
        assert_eq!(details["attribute"], "AXChildren");
    }
}

#[test]
fn expired_deadline_fails_before_accessibility_inventory() {
    let deadline = Deadline::after(0).unwrap();
    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)
        .unwrap()
        .unwrap();
    let error =
        list_surfaces_for_process(identity, deadline).expect_err("expired surface inventory");
    assert_eq!(error.code.as_str(), "TIMEOUT");
}

#[test]
fn only_a_control_with_few_children_is_read_for_an_anchored_surface() {
    assert!(anchors_few_children(Some(1)));
    assert!(anchors_few_children(Some(ANCHOR_CHILD_LIMIT)));
    assert!(!anchors_few_children(Some(0)));
    assert!(!anchors_few_children(Some(ANCHOR_CHILD_LIMIT + 1)));
    assert!(!anchors_few_children(None));
}

#[test]
fn a_dialog_style_window_is_an_alert_only_when_it_is_modal_and_on_screen() {
    assert_eq!(
        classify_surface(Some("AXWindow"), Some("AXDialog"), Some(true), Some(false)),
        Some("alert")
    );
    assert_eq!(
        classify_surface(Some("AXWindow"), Some("AXDialog"), Some(true), None),
        Some("alert")
    );
    assert_eq!(
        classify_surface(Some("AXWindow"), Some("AXDialog"), Some(false), Some(false)),
        None,
        "a fixed-size ordinary window carries the dialog subrole"
    );
    assert_eq!(
        classify_surface(Some("AXWindow"), Some("AXDialog"), None, None),
        None
    );
    assert_eq!(
        classify_surface(Some("AXWindow"), Some("AXDialog"), Some(true), Some(true)),
        None,
        "a minimized window holds nothing up"
    );
}

#[test]
fn sheets_popovers_and_alert_subroles_keep_their_kinds() {
    assert_eq!(
        classify_surface(Some("AXSheet"), None, None, None),
        Some("sheet")
    );
    assert_eq!(
        classify_surface(Some("AXWindow"), Some("AXSheet"), None, None),
        Some("sheet")
    );
    assert_eq!(
        classify_surface(Some("AXPopover"), None, None, None),
        Some("popover")
    );
    assert_eq!(
        classify_surface(Some("AXWindow"), Some("AXAlert"), None, Some(false)),
        Some("alert")
    );
    assert_eq!(
        classify_surface(Some("AXWindow"), Some("AXStandardWindow"), None, None),
        None
    );
}
