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
fn a_failed_inventory_does_not_hide_the_surface_below_the_target() {
    let failed = || Err(agent_desktop_core::AdapterError::timeout("inventory"));

    let kept =
        super::with_anchored(failed(), vec![surface("target/children/0", "popover")]).unwrap();
    assert_eq!(kept.len(), 1);
    assert!(super::with_anchored(failed(), Vec::new()).is_err());

    let both = super::with_anchored(
        Ok(vec![surface("menu", "menu")]),
        vec![surface("target", "popover")],
    )
    .unwrap();
    assert_eq!(
        both.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["menu", "target"]
    );
}

#[test]
fn a_system_dialog_is_listed_as_an_alert_like_a_dialog() {
    assert_eq!(
        super::kind_of(Some("AXWindow"), Some("AXSystemDialog")),
        Some("alert")
    );
    assert_eq!(
        super::kind_of(Some("AXWindow"), Some("AXDialog")),
        Some("alert")
    );
    assert_eq!(super::kind_of(Some("AXSheet"), None), Some("sheet"));
    assert_eq!(
        super::kind_of(Some("AXWindow"), Some("AXStandardWindow")),
        None
    );
}

#[test]
fn a_cut_popover_search_names_popover_unread_on_the_window() {
    let mut surfaces = vec![surface("windows/0", "sheet")];
    mark_popover_unread(&mut surfaces, "windows/0", None);
    mark_popover_unread(&mut surfaces, "windows/1", Some("Doc".into()));
    assert_eq!(surfaces[0].unclassified, ["popover"]);
    assert_eq!(surfaces[1].id, "windows/1");
    assert_eq!(surfaces[1].kind, "window");
    assert_eq!(surfaces[1].unclassified, ["popover"]);
    let cut = AdapterError::new(agent_desktop_core::ErrorCode::ElementNotFound, "cut")
        .with_details(serde_json::json!({ "complete": false }));
    assert!(is_incomplete(&cut));
    assert!(!is_incomplete(&AdapterError::new(
        agent_desktop_core::ErrorCode::ElementNotFound,
        "gone"
    )));
}
