use super::super::golden::assert_golden;
use super::stub::{
    Stub, live_state, masked_by_ref, notes_window, rect, ref_named, snapshot, snapshot_args,
};
use crate::commands::get::{self, GetArgs, GetProperty};
use crate::commands::is_check::{self, IsArgs, IsProperty};
use crate::context::CommandContext;
use crate::refs::refs_test_support::HomeGuard;
use serde_json::{Value, json};

fn get_every_property(stub: &Stub, name: &str) -> Value {
    let ref_id = ref_named(&snapshot(stub, snapshot_args()), name);
    let mut rows = serde_json::Map::new();
    for (key, property) in [
        ("role", GetProperty::Role),
        ("title", GetProperty::Title),
        ("text", GetProperty::Text),
        ("value", GetProperty::Value),
        ("bounds", GetProperty::Bounds),
        ("states", GetProperty::States),
    ] {
        let args = GetArgs {
            ref_id: ref_id.clone(),
            property,
        };
        let data = get::execute(args, stub, &CommandContext::default()).expect("get succeeds");
        rows.insert(key.into(), masked_by_ref(data, &ref_id));
    }
    Value::Object(rows)
}

#[test]
fn get_reports_every_property_from_live_reads() {
    let _guard = HomeGuard::new();
    let stub = Stub {
        state: Some(live_state(&["focused", "selected"])),
        bounds: Some(rect(84.0, 289.0, 200.0, 20.0)),
        value: Some("typed".into()),
        ..Stub::showing(notes_window())
    };

    let rows = get_every_property(&stub, "Title");

    assert_golden(
        rows,
        include_str!("../../../../../../tests/fixtures/contract/observation-get-live.json"),
        &[],
    );
}

#[test]
fn get_reports_null_values_and_recorded_states_without_live_reads() {
    let _guard = HomeGuard::new();
    let stub = Stub::showing(notes_window());

    let rows = get_every_property(&stub, "Save");

    assert_golden(
        rows,
        include_str!("../../../../../../tests/fixtures/contract/observation-get-recorded.json"),
        &[],
    );
}

#[test]
fn is_reports_every_predicate_with_its_applicability() {
    let _guard = HomeGuard::new();
    let stub = Stub {
        state: Some(live_state(&["checked", "focused"])),
        bounds: Some(rect(0.0, 0.0, 10.0, 10.0)),
        ..Stub::showing(notes_window())
    };
    let ref_id = ref_named(&snapshot(&stub, snapshot_args()), "Pinned");
    let mut rows = serde_json::Map::new();

    for (key, property) in [
        ("visible", IsProperty::Visible),
        ("enabled", IsProperty::Enabled),
        ("checked", IsProperty::Checked),
        ("focused", IsProperty::Focused),
        ("expanded", IsProperty::Expanded),
        ("selected", IsProperty::Selected),
    ] {
        let args = IsArgs {
            ref_id: ref_id.clone(),
            property,
        };
        let data = is_check::execute(args, &stub, &CommandContext::default()).expect("is succeeds");
        rows.insert(key.into(), masked_by_ref(data, &ref_id));
    }

    assert_golden(
        json!(rows),
        include_str!("../../../../../../tests/fixtures/contract/observation-is.json"),
        &[],
    );
}
