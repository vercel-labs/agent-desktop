use super::tests::entry;
use super::*;
use agent_desktop_core::Deadline;

#[test]
fn application_menu_resolution_reacquires_invalid_roots_once_without_window_fallback() {
    let pid = i32::try_from(std::process::id()).unwrap();
    let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)
        .unwrap()
        .unwrap();
    for surface in [SnapshotSurface::Menu, SnapshotSurface::Menubar] {
        let mut stored = entry(None, None, None, None);
        stored.process.pid = agent_desktop_core::ProcessId::try_from(pid).unwrap();
        stored.process.process_instance = Some(identity.token());
        stored.source.source_surface = surface;
        for failures in [1, 2] {
            let mut reads = 0;
            let result =
                resolve_with_recovery(&stored, identity, Deadline::after(1000).unwrap(), || {
                    reads += 1;
                    if reads <= failures {
                        Err(crate::tree::surface_read::map_error(
                            "AXRole",
                            accessibility_sys::kAXErrorInvalidUIElement,
                        ))
                    } else {
                        Ok("resolved menu")
                    }
                });
            assert_eq!(reads, 2);
            if failures == 1 {
                assert_eq!(result.unwrap(), "resolved menu");
            } else {
                let error = result.unwrap_err();
                assert!(!error.permits_retry_by_default());
                assert_eq!(error.details.unwrap()["menu_root_reacquired"], true);
            }
        }
    }
}

#[test]
fn ax_read_errors_distinguish_absent_unknown_and_permission_denied() {
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorAttributeUnsupported, kAXErrorCannotComplete,
        kAXErrorInvalidUIElement,
    };

    assert!(
        !crate::tree::resolve::resolve_ax_read::classify_ax_read(
            kAXErrorAttributeUnsupported,
            "AXIdentifier"
        )
        .unwrap()
    );
    for error in [kAXErrorCannotComplete, kAXErrorInvalidUIElement] {
        let classified =
            crate::tree::resolve::resolve_ax_read::classify_ax_read(error, "AXIdentifier")
                .unwrap_err();
        assert_eq!(classified.code, ErrorCode::AppUnresponsive);
        assert_eq!(classified.details.unwrap()["complete"], false);
    }
    let denied = crate::tree::resolve::resolve_ax_read::classify_ax_read(
        kAXErrorAPIDisabled,
        "AXIdentifier",
    )
    .unwrap_err();
    assert_eq!(denied.code, ErrorCode::PermDenied);
}

#[test]
fn an_unknown_identity_is_a_transient_read() {
    let entry = entry(Some(42), Some("w-42"), None, None);
    assert!(
        agent_desktop_core::refs::resolve_errors::identity_unknown_error(&entry)
            .is_transient_incomplete_read()
    );
}
