use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, InteractionLease, InteractionPolicy, SurfaceOpening, WindowInfo};
use std::sync::Mutex;

struct SurfaceAdapter {
    opens: Mutex<Vec<(SnapshotSurface, InteractionPolicy)>>,
    leases: Mutex<u32>,
}

impl ObservationOps for SurfaceAdapter {}
impl ActionOps for SurfaceAdapter {}
impl InputOps for SurfaceAdapter {}

impl SystemOps for SurfaceAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        *self.leases.lock().unwrap() += 1;
        InteractionLease::guarded(deadline, ())
    }

    fn open_system_surface(
        &self,
        surface: SnapshotSurface,
        policy: InteractionPolicy,
        _lease: &InteractionLease,
    ) -> Result<SurfaceOpening, AdapterError> {
        self.opens.lock().unwrap().push((surface, policy));
        match surface {
            SnapshotSurface::Dock => Ok(SurfaceOpening::Present(WindowInfo::for_process(
                &crate::AppInfo {
                    aliases: Vec::new(),
                    name: "Dock".into(),
                    pid: crate::ProcessId::new(7),
                    bundle_id: None,
                    process_instance: Some("instance-7".into()),
                    presentation: None,
                },
            ))),
            SnapshotSurface::Taskbar => Ok(SurfaceOpening::Present(shell_window())),
            _ => Ok(SurfaceOpening::Raised(shell_window())),
        }
    }
}

/// An adapter that keeps the trait default for the open seam, as an adapter
/// without the seam does, but still grants the lease, so the `not_supported` the
/// command surfaces is provably the open seam's own answer and not the
/// lease acquisition failing.
struct LeaseOnlyAdapter {
    leases: Mutex<u32>,
}

impl ObservationOps for LeaseOnlyAdapter {}
impl ActionOps for LeaseOnlyAdapter {}
impl InputOps for LeaseOnlyAdapter {}

impl SystemOps for LeaseOnlyAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        *self.leases.lock().unwrap() += 1;
        InteractionLease::guarded(deadline, ())
    }
}

fn shell_window() -> WindowInfo {
    WindowInfo {
        id: "w-4242".into(),
        title: "Action center".into(),
        app: "ShellExperienceHost.exe".into(),
        pid: crate::ProcessId::new(99),
        process_instance: Some("instance-99".into()),
        bounds: Some(crate::Rect {
            x: 1140.0,
            y: 60.0,
            width: 380.0,
            height: 640.0,
        }),
        state: crate::WindowState {
            is_focused: true,
            accessible: true,
            minimized: Some(false),
            visible: Some(true),
        },
    }
}

fn headed() -> crate::context::CommandContext {
    crate::context::CommandContext::default().with_headed(true)
}

fn adapter() -> SurfaceAdapter {
    SurfaceAdapter {
        opens: Mutex::new(Vec::new()),
        leases: Mutex::new(0),
    }
}

fn shell_window_data() -> serde_json::Value {
    serde_json::json!({
        "id": "w-4242",
        "title": "Action center",
        "app_name": "ShellExperienceHost.exe",
        "pid": 99,
        "process_instance": "instance-99",
        "bounds": { "x": 1140.0, "y": 60.0, "width": 380.0, "height": 640.0 },
        "is_focused": true,
        "accessible": true,
        "minimized": false,
        "visible": true,
    })
}

#[test]
fn a_raised_surface_reports_the_kind_the_window_and_delivered_verified() {
    let adapter = adapter();

    let value = execute(
        OpenSystemSurfaceArgs {
            surface: SnapshotSurface::ActionCenter,
        },
        &adapter,
        &headed(),
    )
    .unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "surface": "action-center",
            "window": shell_window_data(),
            "disposition": { "delivery": "delivered_verified", "retry": "unsafe" },
        })
    );
}

#[test]
fn a_surface_that_was_already_present_reports_not_delivered() {
    let adapter = adapter();

    let value = execute(
        OpenSystemSurfaceArgs {
            surface: SnapshotSurface::Taskbar,
        },
        &adapter,
        &headed(),
    )
    .unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "surface": "taskbar",
            "window": shell_window_data(),
            "disposition": { "delivery": "not_delivered", "retry": "safe" },
        })
    );
}

#[test]
fn the_callers_policy_travels_to_the_adapter_so_the_floor_can_fire() {
    let adapter = adapter();

    execute(
        OpenSystemSurfaceArgs {
            surface: SnapshotSurface::StartMenu,
        },
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    let opens = adapter.opens.lock().unwrap();
    assert_eq!(opens.len(), 1);
    assert_eq!(opens[0].0, SnapshotSurface::StartMenu);
    assert!(
        !opens[0].1.allow_focus_steal,
        "a strict-headless context must hand the adapter a policy that refuses the foreground"
    );
}

#[test]
fn a_headed_policy_is_passed_through_unchanged() {
    let adapter = adapter();

    execute(
        OpenSystemSurfaceArgs {
            surface: SnapshotSurface::SystemTray,
        },
        &adapter,
        &headed(),
    )
    .unwrap();

    let opens = adapter.opens.lock().unwrap();
    assert_eq!(opens.len(), 1);
    assert!(opens[0].1.allow_focus_steal);
    assert!(opens[0].1.allow_cursor_move);
}

#[test]
fn the_command_takes_the_interaction_lease_like_every_desktop_mover() {
    let adapter = adapter();

    execute(
        OpenSystemSurfaceArgs {
            surface: SnapshotSurface::Taskbar,
        },
        &adapter,
        &headed(),
    )
    .unwrap();

    assert_eq!(*adapter.leases.lock().unwrap(), 1);
    assert_eq!(adapter.opens.lock().unwrap().len(), 1);
}

#[test]
fn the_trait_default_answers_not_supported_for_adapters_without_the_surface() {
    let adapter = LeaseOnlyAdapter {
        leases: Mutex::new(0),
    };

    let error = execute(
        OpenSystemSurfaceArgs {
            surface: SnapshotSurface::Dock,
        },
        &adapter,
        &headed(),
    )
    .unwrap_err();

    assert_eq!(
        *adapter.leases.lock().unwrap(),
        1,
        "the lease was granted before the open was attempted"
    );
    let AppError::Adapter(inner) = &error else {
        panic!("expected an adapter error, got: {error:?}");
    };
    assert!(inner.is_default_not_supported("open_system_surface"));
    assert_eq!(error.code(), "PLATFORM_NOT_SUPPORTED");
}

#[test]
fn a_surface_no_window_presents_leaves_the_window_out() {
    let value = execute(
        OpenSystemSurfaceArgs {
            surface: SnapshotSurface::Dock,
        },
        &adapter(),
        &headed(),
    )
    .unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "surface": "dock",
            "disposition": { "delivery": "not_delivered", "retry": "safe" },
        })
    );
}
