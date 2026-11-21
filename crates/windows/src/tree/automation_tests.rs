use super::*;

const E_ACCESSDENIED_HRESULT: i32 = 0x8007_0005_u32 as i32;
const UIA_E_ELEMENTNOTAVAILABLE_HRESULT: i32 = 0x8004_0201_u32 as i32;
const RPC_S_SERVER_UNAVAILABLE_HRESULT: i32 = 0x8007_06BA_u32 as i32;
const UNRECOGNISED_SENTINEL: i32 = 4242;

#[test]
fn a_negative_hresult_formats_with_its_symbolic_name() {
    let error = uia_failure_error(
        UiaFailure::Hresult(E_ACCESSDENIED_HRESULT),
        "read a property",
    );

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)")
    );
}

#[test]
fn a_crate_sentinel_never_prints_a_fabricated_hresult() {
    let error = uia_failure_error(UiaFailure::Sentinel(ERR_TIMEOUT), "read a property");

    assert_eq!(error.code, ErrorCode::Timeout);
    let detail = error.platform_detail.expect("a sentinel carries detail");
    assert!(!detail.contains("0x"), "sentinel detail was {detail}");
    assert!(!error.message.contains("0x"));
}

#[test]
fn an_unrecognised_sentinel_maps_to_internal_rather_than_a_guess() {
    let error = uia_failure_error(
        UiaFailure::Sentinel(UNRECOGNISED_SENTINEL),
        "walk a subtree",
    );

    assert_eq!(error.code, ErrorCode::Internal);
}

#[test]
fn a_disconnected_provider_reports_an_unresponsive_app() {
    let error = uia_failure_error(
        UiaFailure::Hresult(RPC_S_SERVER_UNAVAILABLE_HRESULT),
        "enumerate children",
    );

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}

#[test]
fn only_the_empty_sentinel_counts_as_exhaustion() {
    assert!(UiaFailure::Sentinel(ERR_NONE).is_exhaustion());
    assert!(!UiaFailure::Sentinel(ERR_NOTFOUND).is_exhaustion());
    assert!(!UiaFailure::Hresult(0).is_exhaustion());
    assert!(!UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE_HRESULT).is_exhaustion());
}

#[test]
fn an_unavailable_root_is_a_missing_window_not_a_stale_element() {
    let by_hresult = root_resolution_error(UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE_HRESULT));
    let by_sentinel = root_resolution_error(UiaFailure::Sentinel(ERR_NOTFOUND));

    assert_eq!(by_hresult.code, ErrorCode::WindowNotFound);
    assert_eq!(by_sentinel.code, ErrorCode::WindowNotFound);
    assert!(
        by_hresult
            .platform_detail
            .is_some_and(|detail| detail.contains("UIA_E_ELEMENTNOTAVAILABLE"))
    );
}

#[test]
fn a_root_failure_that_is_not_a_missing_window_keeps_its_own_code() {
    let error = root_resolution_error(UiaFailure::Hresult(E_ACCESSDENIED_HRESULT));

    assert_eq!(error.code, ErrorCode::PermDenied);
}

/// `ref_action.rs` clones adapter error text into session JSONL and into the
/// trace HTML export. A `UiaFailure` carries only an integer, so the error is
/// pinned to its complete static shape: any text a later change appends to the
/// message, the platform detail or the details object fails the comparison.
/// The path that takes caller-supplied text is covered by
/// `a_live_uia_error_never_carries_the_crate_message_verbatim`.
#[test]
fn a_classifier_error_is_exactly_its_static_shape() {
    let error = uia_failure_error(
        UiaFailure::Hresult(E_ACCESSDENIED_HRESULT),
        "read a property",
    );

    assert_eq!(error.message, "UI Automation could not read a property");
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)")
    );
    let details = error.details.expect("the classifier stamps details");
    let mut keys: Vec<&str> = details
        .as_object()
        .expect("details are an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["complete", "retryable"]);
}

#[test]
fn an_empty_window_handle_is_rejected_before_the_com_layer() {
    let deadline = Deadline::standard().expect("a standard deadline");

    let Err(error) = validate_window_handle(0, deadline) else {
        panic!("an empty window handle must be rejected");
    };

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(validate_window_handle(1, deadline).is_ok());
}

#[cfg(not(target_os = "windows"))]
#[test]
fn the_canned_resolver_reports_a_missing_window_for_an_empty_handle() {
    let deadline = Deadline::standard().expect("a standard deadline");

    let Err(error) = root_from_hwnd(0, deadline) else {
        panic!("the canned arm must reject an empty handle");
    };

    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert!(root_from_hwnd(1, deadline).is_ok());
}

#[cfg(target_os = "windows")]
mod windows_only {
    use super::*;
    use crate::system::test_support::apartment::bootstrap;
    use uiautomation::Error as UiaError;

    const LEAK_MARKER: &str = "zzmarkerzz-secret-window-title";

    /// Two calls on one thread hand back the *same* COM object, not two.
    ///
    /// Asserting only that both calls succeed passes just as well for a cache
    /// that was never populated, which would pay a fresh `CoCreateInstance`
    /// and a fresh first-touch serialization on every walk. The interfaces are
    /// compared directly, so a reuse regression fails here.
    #[test]
    fn the_client_is_reusable_within_a_thread() {
        bootstrap();

        let first = automation_client().expect("the first client builds");
        let second = automation_client().expect("the second client builds");

        let first: &windows::Win32::UI::Accessibility::IUIAutomation = first.as_ref();
        let second: &windows::Win32::UI::Accessibility::IUIAutomation = second.as_ref();
        assert_eq!(
            first, second,
            "the second call must return this thread's cached client, not a new instance"
        );
    }

    #[test]
    fn a_hresult_error_splits_onto_the_result_branch() {
        let error = UiaError::from(windows::core::HRESULT(E_ACCESSDENIED_HRESULT));

        assert_eq!(
            failure_of(&error),
            UiaFailure::Hresult(E_ACCESSDENIED_HRESULT)
        );
    }

    #[test]
    fn a_sentinel_error_splits_onto_the_sentinel_branch() {
        let error = UiaError::new(ERR_TIMEOUT, "");

        assert_eq!(failure_of(&error), UiaFailure::Sentinel(ERR_TIMEOUT));
    }

    /// The runtime end-of-list pair the walker depends on, asserted against
    /// the real crate rather than against a reading of it.
    #[test]
    fn a_null_interface_out_param_reports_the_exhaustion_sentinel() {
        let error = UiaError::from(windows::core::Error::empty());

        assert!(failure_of(&error).is_exhaustion());
    }

    #[test]
    fn a_destroyed_window_handle_reports_a_missing_window() {
        bootstrap();
        let deadline = Deadline::standard().expect("a standard deadline");

        let Err(error) = root_from_hwnd(isize::MAX, deadline) else {
            panic!("a handle that addresses no window must not resolve");
        };

        assert!(matches!(
            error.code,
            ErrorCode::WindowNotFound | ErrorCode::InvalidArgs | ErrorCode::ElementNotFound
        ));
    }

    /// Why the client is built from `CUIAutomation8` rather than the CLSID
    /// `UIAutomation::new_direct()` uses: the crate's object does not support
    /// `IUIAutomation2`, so its calls carry no timeout at all. That is the
    /// whole reason this crate does not use it, and the reason there is no
    /// fallback to it.
    #[test]
    fn the_crates_own_client_carries_no_timeout_which_is_why_it_is_not_used() {
        use windows::Win32::UI::Accessibility::{IUIAutomation, IUIAutomation2};
        use windows::core::Interface;
        bootstrap();
        let crate_client = uiautomation::UIAutomation::new_direct().expect("the crate's client");
        let raw: IUIAutomation = crate_client.as_ref().clone();

        assert!(raw.cast::<IUIAutomation2>().is_err());
    }

    /// The client this crate hands out is the bounded one - always, with no
    /// unbounded fallback. A client whose calls cannot time out is refused
    /// rather than quietly accepted, so this assertion holds on every build
    /// the product supports or client creation fails outright.
    #[test]
    fn the_shipped_client_exposes_the_timeouts_it_sets() {
        use windows::Win32::UI::Accessibility::{IUIAutomation, IUIAutomation2};
        use windows::core::Interface;
        bootstrap();
        let client = automation_client().expect("a client");
        let raw: IUIAutomation = client.as_ref().clone();

        let bounded: IUIAutomation2 = raw
            .cast()
            .expect("the shipped client must expose IUIAutomation2");
        assert_eq!(
            unsafe { bounded.ConnectionTimeout() }.expect("a connection timeout is set"),
            crate::tree::automation::CONNECTION_TIMEOUT_MS
        );
    }

    /// UI Automation's client core initializes lazily and not re-entrantly:
    /// concurrent first touches made all but one thread fail instantly with
    /// E_FAIL and "Re-Entrant CheckInit() call, aborting". The accessor
    /// serializes that first touch, so every thread gets a working client.
    #[test]
    fn concurrent_first_touches_do_not_race_uia_lazy_initialization() {
        bootstrap();
        let readers: Vec<_> = (0..4)
            .map(|_| {
                std::thread::spawn(|| {
                    bootstrap();
                    let client = automation_client().expect("a client per thread");
                    client.get_root_element().is_ok()
                })
            })
            .collect();

        for reader in readers {
            assert!(
                reader.join().expect("the reader thread completes"),
                "a healthy desktop-root read was refused by the connection bound"
            );
        }
    }

    #[test]
    fn a_live_uia_error_never_carries_the_crate_message_verbatim() {
        let error = uia_error(&UiaError::new(ERR_TIMEOUT, LEAK_MARKER), "read a property");

        assert!(!error.message.contains(LEAK_MARKER));
        assert!(
            !error
                .platform_detail
                .unwrap_or_default()
                .contains(LEAK_MARKER)
        );
    }
}
