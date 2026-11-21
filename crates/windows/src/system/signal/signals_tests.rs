use super::*;
use agent_desktop_core::{AppInfo, ErrorCode, ProcessId, WindowInfo, WindowState};

use crate::system::test_support::test_time::deadline;

fn window(id: &str, app: &str, pid: u32, instance: &str) -> WindowInfo {
    WindowInfo {
        id: id.to_string(),
        title: String::new(),
        app: app.to_string(),
        pid: ProcessId::from(pid),
        process_instance: Some(instance.to_string()),
        bounds: None,
        state: WindowState {
            is_focused: false,
            accessible: true,
            minimized: None,
            visible: None,
        },
    }
}

fn app_info(name: &str, pid: u32, instance: &str) -> AppInfo {
    AppInfo {
        aliases: Vec::new(),
        name: name.to_string(),
        pid: ProcessId::from(pid),
        bundle_id: None,
        process_instance: Some(instance.to_string()),
        presentation: None,
    }
}

fn inventory(
    windows: Vec<WindowInfo>,
    apps: Vec<AppInfo>,
    windows_complete: bool,
    apps_complete: bool,
) -> SignalWindowInventory {
    SignalWindowInventory {
        windows,
        apps,
        windows_complete,
        apps_complete,
    }
}

#[test]
fn an_unfiltered_assembly_is_complete_with_surfaces_reflecting_no_scan_requested() {
    let source = inventory(
        vec![window("w-1", "one.exe", 100, "gen-a")],
        vec![app_info("one.exe", 100, "gen-a")],
        true,
        true,
    );

    let baseline = assemble(source, &SignalFilter::default(), deadline(10_000))
        .expect("assembling an unfiltered inventory never touches the native surface scan");

    assert!(baseline.completeness.windows);
    assert!(baseline.completeness.apps);
    assert!(
        baseline.surfaces.is_empty(),
        "no app or process was named, so no surface scan runs"
    );
    assert!(
        !baseline.completeness.surfaces,
        "the surfaces bit must report that no scan was requested, not claim a complete \
         scan that never ran"
    );
}

#[test]
fn a_truncated_walk_reports_windows_and_apps_incomplete() {
    let source = inventory(Vec::new(), Vec::new(), false, false);

    let baseline = assemble(source, &SignalFilter::default(), deadline(10_000))
        .expect("a truncated walk is a reported outcome, not an error");

    assert!(
        !baseline.completeness.windows,
        "a walk cut short by the budget must report completeness.windows false"
    );
    assert!(
        !baseline.completeness.apps,
        "a walk cut short by the budget must report completeness.apps false"
    );
}

#[test]
fn a_timeout_error_passes_through_the_boundary_narrowing_unchanged() {
    let error = AdapterError::timeout("Operation exceeded its deadline");

    let narrowed = narrow_to_permitted_codes(error);

    assert_eq!(narrowed.code, ErrorCode::Timeout);
}

#[test]
fn every_non_timeout_code_the_boundary_narrowing_sees_becomes_app_unresponsive() {
    let every_code = [
        ErrorCode::PermDenied,
        ErrorCode::ElementNotFound,
        ErrorCode::AppNotFound,
        ErrorCode::ActionFailed,
        ErrorCode::ActionNotSupported,
        ErrorCode::StaleRef,
        ErrorCode::AmbiguousTarget,
        ErrorCode::WindowNotFound,
        ErrorCode::PlatformNotSupported,
        ErrorCode::InvalidArgs,
        ErrorCode::NotificationNotFound,
        ErrorCode::SnapshotNotFound,
        ErrorCode::PolicyDenied,
        ErrorCode::Internal,
    ];

    for code in every_code {
        let label = code.as_str();
        let narrowed = narrow_to_permitted_codes(AdapterError::new(code, "seam-forced failure"));
        assert_eq!(
            narrowed.code,
            ErrorCode::AppUnresponsive,
            "code {label} must be narrowed to APP_UNRESPONSIVE rather than reaching core \
             unchanged - core aborts the whole wait on any code outside the closed set"
        );
    }
}

#[cfg(target_os = "windows")]
mod windows_only {
    use super::*;

    use agent_desktop_core::{ProcessIdentity, SystemOps};

    use crate::system::test_support::apartment::bootstrap;

    #[test]
    fn an_unfiltered_capture_on_a_live_desktop_is_complete_with_no_surface_scan() {
        bootstrap();

        let baseline = capture_signal_baseline_impl(&SignalFilter::default(), deadline(10_000))
            .expect("an unfiltered capture on a live desktop must succeed");

        assert!(baseline.completeness.windows);
        assert!(baseline.completeness.apps);
        assert!(baseline.surfaces.is_empty());
        assert!(!baseline.completeness.surfaces);
    }

    #[test]
    fn a_process_filter_requests_and_completes_a_surface_scan_of_the_calling_process() {
        bootstrap();
        let own_pid = ProcessId::from(std::process::id());
        let token = crate::system::process_identity::token_for_pid(own_pid)
            .expect("this test process's own token is readable")
            .expect("this test process's own token is a live generation token");
        let filter = SignalFilter {
            app: None,
            process: Some(ProcessIdentity::new(std::process::id(), token)),
        };

        let baseline = capture_signal_baseline_impl(&filter, deadline(10_000))
            .expect("a process-filtered capture scoped to this live process must succeed");

        assert!(
            baseline.completeness.surfaces,
            "a filter naming a process must request and complete a surface scan"
        );
    }

    #[test]
    fn an_already_expired_deadline_is_refused_with_timeout() {
        let expired = Deadline::after(0).expect("a zero-timeout deadline is constructible");

        let error = capture_signal_baseline_impl(&SignalFilter::default(), expired)
            .expect_err("an expired deadline must refuse the capture");

        assert_eq!(error.code, ErrorCode::Timeout);
    }

    #[test]
    fn capture_signal_baseline_reaches_the_windows_override_instead_of_the_not_supported_default() {
        let adapter = crate::adapter::WindowsAdapter::new();

        let result = adapter.capture_signal_baseline(&SignalFilter::default(), deadline(10_000));

        if let Err(error) = result {
            assert_ne!(error.code, ErrorCode::PlatformNotSupported);
        }
    }
}
