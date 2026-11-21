use super::super::golden::assert_golden;
use super::stub::{Stub, leaf, masked, node, notes_window, ref_named, snapshot, snapshot_args};
use crate::SnapshotSurface;
use crate::commands::snapshot::{SnapshotArgs, execute};
use crate::context::{CommandContext, WaitSelector};
use crate::refs::refs_test_support::HomeGuard;
use serde_json::json;

fn waiting_for(query_raw: &str, gone: bool) -> CommandContext {
    CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: query_raw.into(),
        gone,
        timeout_ms: 5_000,
    }))
}

#[test]
fn snapshot_reports_tree_refs_bounds_hints_and_window() {
    let _guard = HomeGuard::new();
    let args = SnapshotArgs {
        include_bounds: true,
        ..snapshot_args()
    };

    let data = snapshot(&Stub::showing(notes_window()), args);

    assert_golden(
        masked(data),
        include_str!("../../../../../../tests/fixtures/contract/observation-window.json"),
        &["snapshot_id"],
    );
}

#[test]
fn skeleton_overview_and_root_drill_down_report_cut_containers_and_the_replaced_subtree() {
    let _guard = HomeGuard::new();
    let stub = Stub::showing(notes_window());
    let skeleton = SnapshotArgs {
        skeleton: true,
        ..snapshot_args()
    };
    let overview = snapshot(&stub, skeleton);
    let root_ref = Some(ref_named(&overview, "Recent"));

    let drill = snapshot(
        &stub,
        SnapshotArgs {
            root_ref,
            ..snapshot_args()
        },
    );

    assert_eq!(drill["snapshot_id"], overview["snapshot_id"]);
    assert_golden(
        json!({ "overview": masked(overview), "root": masked(drill) }),
        include_str!("../../../../../../tests/fixtures/contract/observation-skeleton.json"),
        &["snapshot_id"],
    );
}

#[test]
fn a_cut_short_observation_reports_truncation_and_unproven_identity() {
    let _guard = HomeGuard::new();
    let mut results = leaf("group", "Results");
    results.subtree_truncated = true;
    let mut save = leaf("button", "Save");
    save.presentation.available_actions = vec!["Press".into()];
    let stub = Stub::showing(node("window", "Notes", vec![save, results]));

    let data = snapshot(&stub, snapshot_args());

    assert_golden(
        masked(data),
        include_str!("../../../../../../tests/fixtures/contract/observation-truncated.json"),
        &["snapshot_id"],
    );
}

#[test]
fn an_application_surface_snapshot_omits_the_window() {
    let _guard = HomeGuard::new();
    let mut open = leaf("menuitem", "Open");
    open.presentation.available_actions = vec!["Press".into()];
    let stub = Stub {
        menu: true,
        ..Stub::showing(node("menu", "File", vec![open]))
    };
    let args = SnapshotArgs {
        surface: SnapshotSurface::Menu,
        ..snapshot_args()
    };

    let data = snapshot(&stub, args);

    assert_golden(
        masked(data),
        include_str!("../../../../../../tests/fixtures/contract/observation-menu.json"),
        &["snapshot_id"],
    );
}

#[test]
fn wait_for_reports_the_matched_snapshot_and_wait_for_gone_the_absent_target() {
    let _guard = HomeGuard::new();
    let stub = Stub::showing(notes_window());
    let present = waiting_for("button:Save", false);
    let gone = waiting_for("button:Missing", true);

    let matched = execute(snapshot_args(), &stub, &present).expect("the selector matches");
    let absent = execute(snapshot_args(), &stub, &gone).expect("the selector is gone");

    assert_golden(
        json!({ "matched": masked(matched), "gone": absent }),
        include_str!("../../../../../../tests/fixtures/contract/observation-wait.json"),
        &["snapshot_id", "elapsed_ms"],
    );
}
