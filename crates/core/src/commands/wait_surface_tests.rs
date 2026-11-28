use super::{SurfaceWait, is_kind};
use crate::{SnapshotSurface, SurfaceInfo};

#[test]
fn no_flags_selects_no_surface_wait() {
    assert_eq!(
        SurfaceWait::from_flags(false, false, false, None).unwrap(),
        None
    );
}

#[test]
fn each_flag_maps_to_its_variant() {
    assert_eq!(
        SurfaceWait::from_flags(true, false, false, None).unwrap(),
        Some(SurfaceWait::Menu)
    );
    assert_eq!(
        SurfaceWait::from_flags(false, true, false, None).unwrap(),
        Some(SurfaceWait::MenuClosed)
    );
    assert_eq!(
        SurfaceWait::from_flags(false, false, true, None).unwrap(),
        Some(SurfaceWait::Notification)
    );
}

#[test]
fn conflicting_flags_report_exactly_one_mode_error() {
    for (menu, menu_closed, notification) in [
        (true, true, false),
        (true, false, true),
        (false, true, true),
        (true, true, true),
    ] {
        let err = SurfaceWait::from_flags(menu, menu_closed, notification, None).unwrap_err();

        assert_eq!(err.code(), "INVALID_ARGS");
        assert_eq!(err.to_string(), "wait accepts exactly one mode");
        assert!(err.suggestion().is_some());
    }
}

#[test]
fn a_surface_kind_selects_an_open_surface_wait_and_conflicts_with_a_flag() {
    assert_eq!(
        SurfaceWait::from_flags(false, false, false, Some("popover")).unwrap(),
        Some(SurfaceWait::Open(Some(SnapshotSurface::Popover)))
    );
    assert_eq!(
        SurfaceWait::from_flags(false, false, false, Some("any")).unwrap(),
        Some(SurfaceWait::Open(None))
    );
    let conflict = SurfaceWait::from_flags(true, false, false, Some("sheet")).unwrap_err();
    assert_eq!(conflict.code(), "INVALID_ARGS");
    let unknown = SurfaceWait::from_flags(false, false, false, Some("window")).unwrap_err();
    assert_eq!(unknown.code(), "INVALID_ARGS");
}

#[test]
fn a_listed_surface_matches_its_kind_and_a_context_menu_is_a_menu() {
    let listed = |kind: &str| SurfaceInfo {
        id: "windows/0".into(),
        kind: kind.into(),
        title: None,
        item_count: None,
        unclassified: Vec::new(),
    };
    assert!(is_kind(&listed("popover"), Some(SnapshotSurface::Popover)));
    assert!(!is_kind(&listed("sheet"), Some(SnapshotSurface::Popover)));
    assert!(is_kind(
        &listed("context_menu"),
        Some(SnapshotSurface::Menu)
    ));
    assert!(is_kind(&listed("alert"), None));
    assert!(is_kind(&listed("context_menu"), None));
    assert!(
        !is_kind(&listed("window"), None),
        "any must not match a plain window, which Linux and Windows list for every window"
    );
}
