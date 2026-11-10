use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, RefEntry};

use crate::tree::resolve::resolve_fake::{FRAME_PATH, Fake, deadline, node};
use crate::tree::resolve::{Source, resolve};
use crate::tree::vocabulary::role_numbers::{LIST, LIST_ITEM, PANEL, PUSH_BUTTON};

fn window() -> Fake {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/toolbar", node(PANEL, "toolbar", 800, 40));
    fake.add("/toolbar", "/ok1", node(PUSH_BUTTON, "OK", 30, 30));
    fake.add("/toolbar", "/ok2", node(PUSH_BUTTON, "OK", 30, 30));
    fake.add(FRAME_PATH, "/files", node(LIST, "files", 800, 500));
    fake.add("/files", "/row1", node(LIST_ITEM, "notes.txt", 800, 20));
    fake
}

fn token_entry(fake: &Fake, path: &str) -> RefEntry {
    let mut entry = fake.entry_of(path, false);
    entry.identity.retained_object = Some(fake.token_of(path));
    entry
}

fn reason(error: &AdapterError) -> (&str, Option<u64>) {
    let details = error.details.as_ref().expect("the refusal has details");
    (
        details["reason"].as_str().unwrap_or_default(),
        details.get("depth").and_then(serde_json::Value::as_u64),
    )
}

#[test]
fn identical_twins_each_resolve_to_their_own_element() {
    let mut fake = window();
    for path in ["/ok1", "/ok2"] {
        let entry = token_entry(&fake, path);
        let element = resolve(&mut fake, &entry, deadline()).expect("the twin resolves");
        assert_eq!(element.path(), path);
        let live = fake.evidence(&element).expect("the element reads");
        assert_eq!(
            live.identifiers.retained_object(),
            entry.identity.retained_object.as_deref()
        );
    }
}

#[test]
fn a_recycled_row_under_another_name_is_stale() {
    let mut fake = window();
    let entry = token_entry(&fake, "/row1");
    fake.nodes.get_mut("/row1").expect("row").read = node(LIST_ITEM, "other.txt", 800, 20);
    let error = resolve(&mut fake, &entry, deadline()).expect_err("refused");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(reason(&error), ("identity", None));
}

#[test]
fn a_row_moved_out_of_its_list_is_stale_at_that_link() {
    let mut fake = window();
    let entry = token_entry(&fake, "/row1");
    fake.nodes.get_mut("/files").expect("list").children.clear();
    let error = resolve(&mut fake, &entry, deadline()).expect_err("refused");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(reason(&error), ("chain", Some(2)));
}

#[test]
fn a_closed_window_whose_objects_still_answer_is_stale_at_the_application_root() {
    let mut fake = window();
    let entry = token_entry(&fake, "/ok1");
    fake.nodes
        .get_mut(crate::system::REGISTRY_ROOT_PATH)
        .expect("root")
        .children
        .clear();
    let error = resolve(&mut fake, &entry, deadline()).expect_err("refused");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(reason(&error), ("chain", Some(0)));
}

#[test]
fn an_unanswered_or_unread_link_is_not_called_stale() {
    let mut fake = window();
    let entry = token_entry(&fake, "/ok1");
    fake.faults
        .level_errors
        .insert("/toolbar".to_owned(), AdapterError::timeout("no answer"));
    let timed_out = resolve(&mut fake, &entry, deadline()).expect_err("refused");
    assert_eq!(timed_out.code, ErrorCode::Timeout);

    let mut fake = window();
    fake.nodes
        .get_mut("/toolbar")
        .expect("toolbar")
        .children_unread = true;
    let unread = resolve(&mut fake, &entry, deadline()).expect_err("refused");
    let details = unread.details.as_ref().expect("details");
    assert_eq!(details["kind"], "resolution_identity_unknown");
    assert_eq!(details["retryable"], true);
}

#[test]
fn a_replaced_window_or_a_stopped_process_settles_before_the_chain_is_read() {
    let mut fake = window();
    let entry = token_entry(&fake, "/ok1");
    fake.faults.gate_error = Some(super::super::resolve_errors::window_replaced());
    let error = resolve(&mut fake, &entry, deadline()).expect_err("refused");
    assert_eq!(
        error.details.as_ref().expect("details")["kind"],
        "resolve_window_replaced"
    );
    assert_eq!(fake.reads.levels, 0);

    let mut fake = window();
    fake.stopped = Some("files".to_owned());
    let error = resolve(&mut fake, &entry, deadline()).expect_err("stopped");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(fake.reads.gate, 0);
}

fn with_token_field(
    entry: &RefEntry,
    index: usize,
    replace: impl FnOnce(&str) -> String,
) -> RefEntry {
    let token = entry.identity.retained_object.as_deref().expect("a token");
    let mut fields: Vec<String> = token.split('|').map(str::to_owned).collect();
    fields[index] = replace(&fields[index]);
    let mut changed = entry.clone();
    changed.identity.retained_object = Some(fields.join("|"));
    changed
}

fn assert_stale_at(error: &AdapterError, expected_reason: &str) {
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(reason(error), (expected_reason, None));
}

#[test]
fn a_token_from_another_bus_instance_is_stale_before_any_window_is_read() {
    let mut fake = window();
    let entry = token_entry(&fake, "/ok1");
    fake.guid = "restarted-bus".to_owned();
    let error = resolve(&mut fake, &entry, deadline()).expect_err("refused");
    assert_stale_at(&error, "bus");
    assert_eq!(fake.reads.gate, 0);
}

#[test]
fn a_token_for_another_frame_process_or_bus_name_is_stale_at_the_window() {
    let fake = window();
    let entry = token_entry(&fake, "/ok1");
    let other_frame =
        with_token_field(&entry, 5, |suffixes| suffixes.replacen("frame", "other", 1));
    let other_process = with_token_field(&entry, 1, |_| "41".to_owned());
    let other_bus = with_token_field(&entry, 2, |_| ":1.9".to_owned());
    for changed in [other_frame, other_process, other_bus] {
        let mut fake = window();
        let error = resolve(&mut fake, &changed, deadline()).expect_err("refused");
        assert_stale_at(&error, "window");
        assert_eq!(fake.reads.levels, 0);
    }
}

#[test]
fn a_ref_with_no_token_or_a_foreign_token_is_refused_without_a_read() {
    let mut fake = window();
    let tokenless = fake.entry_of("/ok1", false);
    let mut foreign = tokenless.clone();
    foreign.identity.retained_object = Some("uia1:not-a-linux-token".to_owned());
    for entry in [tokenless, foreign] {
        let error = resolve(&mut fake, &entry, deadline()).expect_err("refused");
        assert_eq!(error.code, ErrorCode::StaleRef);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert_eq!(
            error.details.as_ref().expect("details")["kind"],
            "element_token_required"
        );
    }
    assert_eq!(fake.reads.gate, 0);
    assert_eq!(fake.reads.levels, 0);
}
