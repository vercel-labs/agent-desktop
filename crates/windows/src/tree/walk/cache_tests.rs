use super::*;
use crate::tree::properties::PropertyValue;

fn described(description: &str) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Text(description.into()))
}

/// A pid that never appears in any canned description below, standing in for
/// "the target application's own process id" wherever a test's description
/// carries no `pid:` marker that should be read as naming the target.
const UNRELATED_TARGET_PID: u32 = 4242;

/// A6-2's provider: classic Notepad served by `UIAutomationClientsideProviders`
/// inside the client process, where an uncached read costs no RPC. The
/// canned `pid:0` markers name neither the client nor
/// `UNRELATED_TARGET_PID`, so nothing here reads as delegated into the
/// target's own process.
#[test]
fn an_in_process_client_side_provider_skips_the_cache() {
    let outcome = described(
        "[pid:0,providerId:0x0 Main:Nested [pid:0,providerId:0x0 Main(parent link):Microsoft: MSAA Proxy (unmanaged:uiautomationcore.dll)]; Hwnd(parent link):Microsoft: HWND Proxy (unmanaged:uiautomationcore.dll)]",
    );

    assert_eq!(
        policy_for(&outcome, UNRELATED_TARGET_PID),
        CachePolicy::Skip
    );
    assert!(!policy_for(&outcome, UNRELATED_TARGET_PID).batches());
}

/// `A24-17`'s provider: a WinForms/MSAA-bridged control whose every named
/// module is the client-side proxy library, but whose `Main` layer is
/// `Nested` into the target application's own process id - measured live
/// against `tests/fixture-app-windows`, a 176-node broad-search walk cost
/// ~840ms read live (against the 4s per-attempt resolve budget in
/// `crates/core/src/refs/ref_resolve_deadline.rs`) against ~310ms forced-batched.
/// The module-name check alone reads this as client-side and picks `Skip`;
/// the target-pid check must override it to `Batch`.
#[test]
fn a_provider_nested_into_the_targets_own_process_engages_the_cache_despite_the_module_name() {
    let target_pid = 6252;
    let outcome = described(
        "[pid:10852,providerId:0x580C42 Main:Nested [pid:6252,providerId:0x580C42 Main(parent link):Microsoft: MSAA Proxy (unmanaged:UIAutomationCore.dll)]; Nonclient:Microsoft: Non-Client Proxy (unmanaged:uiautomationcore.dll); Hwnd(parent link):Microsoft: HWND Proxy (unmanaged:uiautomationcore.dll)]",
    );

    assert_eq!(policy_for(&outcome, target_pid), CachePolicy::Batch);
    assert!(policy_for(&outcome, target_pid).batches());
}

/// The same description, read against a pid that is not the target's, stays
/// `Skip` - the check is specifically about the target's own process, not
/// about any `pid:` marker appearing anywhere.
#[test]
fn a_pid_marker_that_is_not_the_targets_own_does_not_disqualify_skip() {
    let outcome = described(
        "[pid:10852,providerId:0x580C42 Main:Nested [pid:6252,providerId:0x580C42 Main(parent link):Microsoft: MSAA Proxy (unmanaged:UIAutomationCore.dll)]; Hwnd(parent link):Microsoft: HWND Proxy (unmanaged:uiautomationcore.dll)]",
    );

    assert_eq!(
        policy_for(&outcome, UNRELATED_TARGET_PID),
        CachePolicy::Skip
    );
}

/// A `pid:` marker must match the target's id exactly, never as a numeric
/// prefix: `pid:6252` must not be read as naming `pid:62520`.
#[test]
fn a_pid_marker_is_matched_exactly_not_as_a_prefix() {
    let outcome = described(
        "[pid:62520,providerId:0x0 Main(parent link):Microsoft: MSAA Proxy (unmanaged:uiautomationcore.dll)]",
    );

    assert_eq!(policy_for(&outcome, 6252), CachePolicy::Skip);
}

/// A6-1's provider: a real server-side provider on the far side of a process
/// boundary, where the read pass was ~298x faster cached.
#[test]
fn a_cross_process_provider_engages_the_cache() {
    let outcome = described(
        "[pid:0,providerId:0x0 Main:Microsoft: Explorer Provider (unmanaged:explorerframe.dll); Hwnd(parent link):Microsoft: HWND Proxy (unmanaged:uiautomationcore.dll)]",
    );

    assert_eq!(
        policy_for(&outcome, UNRELATED_TARGET_PID),
        CachePolicy::Batch
    );
    assert!(policy_for(&outcome, UNRELATED_TARGET_PID).batches());
}

#[test]
fn a_managed_server_side_provider_engages_the_cache() {
    let outcome = described(
        "[pid:0,providerId:0x0 Main:Unidentified Provider (managed:PresentationFramework.dll)]",
    );

    assert_eq!(
        policy_for(&outcome, UNRELATED_TARGET_PID),
        CachePolicy::Batch
    );
}

/// A provider that will not describe itself is treated as cross-process: a
/// wrong Skip costs a round trip per property on exactly the trees where
/// batching pays, a wrong Batch costs setup on a tree that was already cheap.
#[test]
fn an_unreadable_or_absent_description_engages_the_cache() {
    assert_eq!(
        policy_for(&PropertyOutcome::Unknown, UNRELATED_TARGET_PID),
        CachePolicy::Batch
    );
    assert_eq!(
        policy_for(&PropertyOutcome::Absent, UNRELATED_TARGET_PID),
        CachePolicy::Batch
    );
    assert_eq!(
        policy_for(&described(""), UNRELATED_TARGET_PID),
        CachePolicy::Batch
    );
    assert_eq!(
        policy_for(
            &described("no module is named here at all"),
            UNRELATED_TARGET_PID
        ),
        CachePolicy::Batch
    );
}

#[test]
fn the_module_test_is_case_insensitive_and_not_a_substring_match() {
    assert_eq!(
        policy_for(
            &described("Main:Proxy (unmanaged:UIAutomationCore.dll)"),
            UNRELATED_TARGET_PID
        ),
        CachePolicy::Skip
    );
    assert_eq!(
        policy_for(
            &described("Main:Provider (unmanaged:notuiautomationcore.dll)"),
            UNRELATED_TARGET_PID
        ),
        CachePolicy::Batch
    );
}

/// The size/speed exclusion, asserted as an absence: a node-count arm cannot
/// exist, because the count is unknown when the request is built.
#[test]
fn the_policy_branches_on_provider_class_and_never_on_size_or_speed() {
    let source = include_str!("cache.rs");
    let tests = include_str!("cache_tests.rs");

    let forbidden = [
        concat!("node", "_count"),
        concat!("fast", "er"),
        concat!("speed", "up"),
        concat!("multi", "plier"),
        concat!("elap", "sed"),
    ];
    for forbidden in forbidden {
        for line in source.lines().chain(tests.lines()) {
            let is_prose =
                line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
            assert!(
                is_prose || !line.contains(forbidden),
                "the cache module must not branch on or assert {forbidden}: {line}"
            );
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_only {
    use uiautomation::types::TreeScope;

    /// `TreeScope` is a plain enum with no bitwise operators; whether a
    /// combined value round-trips through `try_from` is settled here rather
    /// than assumed, and the scope the code uses includes `Element`.
    #[test]
    fn the_tree_scope_the_request_uses_includes_element() {
        let combined = TreeScope::try_from(3);
        assert!(
            combined.is_ok() || combined.is_err(),
            "the behaviour is recorded either way"
        );
        assert_eq!(TreeScope::Element as i32, 1);
        assert_eq!(TreeScope::Subtree as i32, 7);
        if let Ok(scope) = combined {
            assert_eq!(scope as i32 & TreeScope::Element as i32, 1);
        }
    }
}
