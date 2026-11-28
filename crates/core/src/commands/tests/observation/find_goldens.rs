use super::super::golden::assert_golden;
use super::stub::{Stub, masked, notes_window};
use crate::SnapshotSurface;
use crate::commands::find::{FindArgs, FindFilterArgs, FindSelectionArgs, execute};
use crate::commands::query::{parse_selector, tree_has_match, validate_selector};
use crate::context::CommandContext;
use crate::refs::refs_test_support::HomeGuard;
use serde_json::{Map, Value, json};

fn find(role: Option<&str>, text: Option<&str>, count: bool, first: bool) -> Value {
    let _guard = HomeGuard::new();
    let args = FindArgs {
        app: None,
        window_id: None,
        root: None,
        surface: SnapshotSurface::Window,
        filter: FindFilterArgs {
            role: role.map(Into::into),
            name: None,
            description: None,
            native_id: None,
            value: None,
            text: text.map(Into::into),
            exact: false,
        },
        states: Vec::new(),
        selection: FindSelectionArgs {
            count,
            first,
            last: false,
            nth: None,
            limit: None,
        },
        timeout_ms: None,
    };
    let stub = Stub::showing(notes_window());
    execute(args, &stub, &CommandContext::default()).expect("the find succeeds")
}

#[test]
fn find_lists_matches_with_refs_geometry_value_and_states() {
    let data = find(None, Some("note"), false, false);

    assert_golden(
        masked(data),
        include_str!("../../../../../../tests/fixtures/contract/observation-find-all.json"),
        &["snapshot_id"],
    );
}

#[test]
fn find_count_reports_only_the_number() {
    let data = find(Some("button"), None, true, false);

    assert_golden(
        data,
        include_str!("../../../../../../tests/fixtures/contract/observation-find-count.json"),
        &[],
    );
}

#[test]
fn find_first_reports_one_match_and_its_snapshot() {
    let data = find(Some("button"), None, false, true);

    assert_golden(
        masked(data),
        include_str!("../../../../../../tests/fixtures/contract/observation-find-first.json"),
        &["snapshot_id"],
    );
}

#[test]
fn find_without_a_match_reports_roles_present_only_when_a_role_was_asked() {
    let all = find(Some("link"), None, false, false);
    let first = find(Some("link"), None, false, true);
    let no_role = find(None, Some("absent text"), false, false);

    assert_golden(
        json!({ "all": masked(all), "first": masked(first), "no_role": masked(no_role) }),
        include_str!("../../../../../../tests/fixtures/contract/observation-find-none.json"),
        &["snapshot_id"],
    );
}

#[test]
fn query_selectors_parse_validate_and_match_the_notes_window() {
    let tree = notes_window();
    let rows: Map<String, Value> = ["button:Save", ":Last edited", "button", "", "textfield:a:b"]
        .into_iter()
        .map(|raw| {
            let query = parse_selector(raw);
            let row = json!({
                "query": query,
                "tree_match": tree_has_match(&tree, &query),
                "valid": validate_selector(raw).is_ok(),
            });
            (raw.to_owned(), row)
        })
        .collect();

    assert_golden(
        Value::Object(rows),
        include_str!("../../../../../../tests/fixtures/contract/observation-query.json"),
        &[],
    );
}
