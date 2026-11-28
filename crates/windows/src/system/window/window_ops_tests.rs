use super::*;
use agent_desktop_core::Rect;

use crate::system::test_support::test_time::deadline;

#[test]
fn the_filter_excludes_invisible_zero_sized_cloaked_and_tool_windows() {
    let sample = EnumeratedWindow {
        handle: std::ptr::null_mut(),
        visible: true,
        iconic: false,
        cloaked: false,
        tool: false,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 40.0,
        },
    };
    assert!(passes_filter(&sample));

    assert!(!passes_filter(&EnumeratedWindow {
        visible: false,
        ..sample
    }));
    assert!(!passes_filter(&EnumeratedWindow {
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        ..sample
    }));
    assert!(!passes_filter(&EnumeratedWindow {
        cloaked: true,
        ..sample
    }));
    assert!(!passes_filter(&EnumeratedWindow {
        tool: true,
        ..sample
    }));
}

#[test]
fn the_window_id_is_the_hwnd_with_the_w_prefix() {
    let parsed = parse_handle("w-1000");
    assert_eq!(parsed as usize, 1000);
}

#[cfg(target_os = "windows")]
mod windows_only {
    use super::*;

    /// The deadline this entry point accepts is a real budget, not an
    /// ignored parameter. An already-expired deadline must refuse before any
    /// native enumeration runs at all - invert-verifiable by dropping the
    /// `ensure_remaining` preamble in `retry_transient_window_race` and watching
    /// this assertion fail as enumeration proceeds anyway.
    #[test]
    fn an_already_expired_deadline_returns_timeout_without_enumerating() {
        enumeration_calls::take();
        let expired = Deadline::after(0).expect("a zero-timeout deadline is constructible");

        let error = list_windows_live(&WindowFilter::default(), expired)
            .expect_err("an expired deadline must refuse the listing");

        assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
        assert_eq!(
            enumeration_calls::take(),
            0,
            "an already-expired deadline must perform no native enumeration"
        );
    }

    /// A mid-walk identity race is absorbed internally rather than surfacing
    /// to `wait_for_window`, whose retryable set does not include
    /// `WINDOW_NOT_FOUND`.
    ///
    /// This forces exactly `FORCED_RACES` inconsistencies. The filter names
    /// an application no window belongs to, so no listed window reaches
    /// `re_verify` and the live desktop cannot inject a genuine identity race
    /// on top of the forced ones: the attempt count is exactly the forced
    /// races plus the attempt that succeeds, and a regression to unbounded
    /// or absent retries changes it.
    #[test]
    fn a_forced_transient_inconsistency_is_retried_and_succeeds_once_it_clears() {
        crate::system::test_support::apartment::ensure_test_apartment();
        enumeration_calls::take();

        const FORCED_RACES: usize = 2;
        let filter = WindowFilter {
            focused_only: false,
            app: Some("agent-desktop-no-such-application.exe".to_string()),
        };
        let outcome = force_window_not_found::with(FORCED_RACES, || {
            list_windows_live(&filter, deadline(10_000))
        });

        assert!(
            outcome.is_ok(),
            "the listing must succeed once the forced inconsistency clears"
        );
        assert_eq!(
            enumeration_calls::take(),
            FORCED_RACES + 1,
            "the {FORCED_RACES} forced-race attempts plus the attempt that succeeded"
        );
    }

    /// A race that survives the whole retry budget still returns the exact
    /// refusal the shipped inventory has always reported, so `wait --window`
    /// and every other caller of this refusal see no change in kind - only
    /// in how often it is reached.
    #[test]
    fn a_persistent_inconsistency_exhausts_the_retry_budget_and_still_refuses() {
        crate::system::test_support::apartment::ensure_test_apartment();
        enumeration_calls::take();

        let outcome = force_window_not_found::with(
            crate::system::window::listing_retry::LISTING_RACE_ATTEMPTS as usize * 2,
            || list_windows_live(&WindowFilter::default(), deadline(10_000)),
        );

        let error = outcome.expect_err("a persistent inconsistency must exhaust the retry budget");
        assert_eq!(error.code, agent_desktop_core::ErrorCode::WindowNotFound);
        assert_eq!(
            enumeration_calls::take(),
            crate::system::window::listing_retry::LISTING_RACE_ATTEMPTS as usize,
            "every attempt performs its own walk"
        );
    }
}
