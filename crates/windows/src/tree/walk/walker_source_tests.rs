use crate::tree::test_support::assert_source_forbids;

/// The crate's own `get_children` retires end-of-siblings and a cross-process
/// fault through the same arm; `SetFocus` moves the desktop foreground (A3-4)
/// and is therefore not headless; ref allocation belongs to core alone.
#[test]
fn the_walk_issues_no_banned_call() {
    let banned = [
        concat!("get_", "children"),
        concat!("Set", "Focus"),
        concat!("allocate_", "refs"),
    ];
    assert_source_forbids(
        &[
            ("walker.rs", include_str!("walker.rs")),
            ("walker_enumerate.rs", include_str!("walker_enumerate.rs")),
            ("walker_source.rs", include_str!("walker_source.rs")),
            ("walker_tests.rs", include_str!("walker_tests.rs")),
            ("walker_fake.rs", include_str!("walker_fake.rs")),
            (
                "walker_source_tests.rs",
                include_str!("walker_source_tests.rs"),
            ),
        ],
        |line| banned.iter().any(|call| line.contains(call)),
    );
}

/// The bans on instantiating a pattern or keying on a display string, over
/// the files that would be tempted to break them.
///
/// Pattern-derived state is read as ordinary batched properties. The
/// crate-idiomatic route is `get_pattern`, a COM `QueryInterface` per node per
/// pattern - roughly 1,100 extra cross-process calls on a 220-node tree
/// against a walk tuned to batch - and `UICacheRequest::add_pattern` is the
/// same cost moved into the request. Neither exists here, and this is what
/// keeps it that way.
///
/// `LocalizedControlType` is banned as a map key separately: Microsoft
/// documents it as an OS-locale-dependent or provider-chosen display string,
/// so a role keyed on it breaks on a non-English Windows. The ban is scoped to
/// the vocabulary files that *key* on a control type - `roles.rs`,
/// `actions.rs`, `states.rs`. `property_ids.rs` declares and reads the
/// property here (A16-5 measured `LocalizedControlType` populated on every
/// control type, and it feeds the `role_description` descriptor), which is
/// not keying.
#[test]
fn the_vocabulary_instantiates_no_pattern_and_keys_on_no_display_string() {
    let vocabulary_banned = [
        concat!("get_", "pattern"),
        concat!("add_", "pattern"),
        concat!("Localized", "ControlType"),
    ];
    assert_source_forbids(
        &[
            ("roles.rs", include_str!("../roles.rs")),
            ("actions.rs", include_str!("../actions.rs")),
            ("states.rs", include_str!("../states.rs")),
        ],
        |line| vocabulary_banned.iter().any(|call| line.contains(call)),
    );
    let pattern_banned = [concat!("get_", "pattern"), concat!("add_", "pattern")];
    assert_source_forbids(
        &[
            (
                "property_ids.rs",
                include_str!("../properties/property_ids.rs"),
            ),
            ("properties.rs", include_str!("../properties/mod.rs")),
            ("cache.rs", include_str!("cache.rs")),
            (
                "element_properties.rs",
                include_str!("../properties/element_properties.rs"),
            ),
        ],
        |line| pattern_banned.iter().any(|call| line.contains(call)),
    );
}

#[cfg(not(target_os = "windows"))]
mod non_windows_only {
    use super::super::*;
    use crate::tree::element::{CannedElement, UIAElement};
    use crate::tree::walk::walker_fake::{budget, window};
    use agent_desktop_core::ObservationRoot;

    /// The canned arm exists so every module that calls the walk compiles and
    /// runs off Windows. It enumerates nothing, which is the exhaustion answer,
    /// so its walk is a single complete node.
    #[test]
    fn the_canned_source_walks_a_single_complete_node() {
        let window = window();
        let root = UIAElement::from(CannedElement);

        let outcome = walk_uia_subtree(&root, &ObservationRoot::Window(&window), budget(10))
            .expect("the canned walk assembles an observation");

        assert!(outcome.tree.is_complete());
        assert!(outcome.failures.is_empty());
        assert!(outcome.tree.into_accessibility_tree().is_ok());
    }
}
