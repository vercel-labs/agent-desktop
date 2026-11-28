use super::*;
use crate::AdapterError;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};

struct SurfaceInventoryAdapter {
    surfaces: Vec<crate::SurfaceInfo>,
}

impl ObservationOps for SurfaceInventoryAdapter {
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _deadline: crate::Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Ok(query.answer(vec![crate::AppInfo {
            aliases: Vec::new(),
            name: "SurfaceApp".into(),
            pid: crate::ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("test-instance".into()),
            presentation: None,
        }]))
    }

    fn list_surfaces(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::SurfaceInfo>, AdapterError> {
        Ok(self.surfaces.clone())
    }
}

impl ActionOps for SurfaceInventoryAdapter {}

impl InputOps for SurfaceInventoryAdapter {}

impl SystemOps for SurfaceInventoryAdapter {
    fn supported_surfaces(&self) -> Vec<crate::SnapshotSurface> {
        vec![
            crate::SnapshotSurface::Popover,
            crate::SnapshotSurface::Sheet,
            crate::SnapshotSurface::Alert,
            crate::SnapshotSurface::Menu,
        ]
    }
}

fn unread_window(kind: &str) -> crate::SurfaceInfo {
    crate::SurfaceInfo {
        id: "w-7".into(),
        kind: "window".into(),
        title: Some("Document".into()),
        item_count: None,
        unclassified: vec![kind.into()],
    }
}

fn read_window() -> crate::SurfaceInfo {
    crate::SurfaceInfo {
        id: "w-7".into(),
        kind: "window".into(),
        title: Some("Document".into()),
        item_count: None,
        unclassified: Vec::new(),
    }
}

fn wait_for(kind: &str, surfaces: Vec<crate::SurfaceInfo>) -> AppError {
    use super::test_support::wait_args;
    let adapter = SurfaceInventoryAdapter { surfaces };
    execute(
        WaitArgs {
            mode: WaitModeArgs {
                surface: Some(
                    SurfaceWait::from_flags(false, false, false, Some(kind))
                        .unwrap()
                        .expect("one surface wait"),
                ),
                ..wait_args().mode
            },
            app: Some("SurfaceApp".into()),
            timeout_ms: 30,
            ..wait_args()
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err()
}

fn details(error: &AppError) -> Value {
    let AppError::Adapter(error) = error else {
        panic!("a surface timeout is an adapter timeout, got {error:?}");
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    error.details.clone().expect("timeout details")
}

#[test]
fn a_surface_kind_the_inventory_could_not_read_is_not_reported_as_absent() {
    for kind in ["popover", "sheet", "menu"] {
        let error = wait_for(kind, vec![unread_window(kind)]);
        let message = error.to_string();
        assert!(message.contains("could not be read"), "{kind}: {message}");
        assert!(
            !message.contains("No "),
            "{kind} claims absence for a surface that was never read: {message}"
        );
        let suggestion = error.suggestion().unwrap_or_default();
        assert!(
            suggestion.contains("may still present it"),
            "{kind}: {suggestion}"
        );
        assert!(
            !suggestion.contains("showed no such surface"),
            "{kind}: {suggestion}"
        );
        let details = details(&error);
        assert_eq!(details["unread"], true, "{kind}: {details}");
        assert_eq!(details["predicate"], "surface");
        assert_eq!(details["surface"], kind);
        assert_eq!(details["last_observed"][0]["id"], "w-7");
    }
}

#[test]
fn any_surface_is_unread_when_a_listed_surface_reports_any_open_kind() {
    let error = wait_for("any", vec![unread_window("menu")]);
    assert_eq!(details(&error)["unread"], true);
}

#[test]
fn a_read_inventory_without_the_surface_still_reports_a_plain_timeout() {
    let error = wait_for("popover", vec![read_window()]);
    assert!(
        error.to_string().contains("No popover surface was open"),
        "{error}"
    );
    let details = details(&error);
    assert!(details.get("unread").is_none(), "{details}");
    assert_eq!(details["last_observed"][0]["id"], "w-7");
}

#[test]
fn a_matching_surface_returns_before_any_timeout() {
    use super::test_support::wait_args;
    let adapter = SurfaceInventoryAdapter {
        surfaces: vec![crate::SurfaceInfo {
            id: "w-7".into(),
            kind: "popover".into(),
            title: Some("Filters".into()),
            item_count: None,
            unclassified: Vec::new(),
        }],
    };
    let value = execute(
        WaitArgs {
            mode: WaitModeArgs {
                surface: Some(SurfaceWait::Open(Some(crate::SnapshotSurface::Popover))),
                ..wait_args().mode
            },
            app: Some("SurfaceApp".into()),
            timeout_ms: 30,
            ..wait_args()
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(value["surface"]["id"], "w-7");
}
