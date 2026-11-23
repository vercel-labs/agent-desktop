//! The completeness rule through the lookups that apply it. A test installs a
//! workspace snapshot with no app and one live processless record, which the
//! inventories then read in place of the AppKit bridge.

use agent_desktop_core::{AdapterError, ErrorCode, WindowFilter};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::time::{Duration, Instant};

thread_local! {
    static SNAPSHOT: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// The text of a lookup for an app that is not running.
pub(crate) const ABSENT: &str = "Agent Desktop Absent App";

/// The workspace snapshot installed on this thread, if any.
pub(crate) fn workspace_snapshot() -> Option<Vec<u8>> {
    SNAPSHOT.with_borrow(Clone::clone)
}

/// Removes the installed snapshot when dropped.
pub(crate) struct SnapshotFixture;

impl Drop for SnapshotFixture {
    fn drop(&mut self) {
        SNAPSHOT.set(None);
    }
}

/// Installs a snapshot with no app and `record` as its only skipped record.
pub(crate) fn install_phantom(record: &Value) -> SnapshotFixture {
    let snapshot = json!({
        "applications": [], "frontmost_pid": 0, "frontmost_launch_time": null,
        "skipped": [record],
    });
    SNAPSHOT.set(Some(serde_json::to_vec(&snapshot).unwrap()));
    SnapshotFixture
}

/// A live record that names its app by every alias.
pub(crate) fn phantom() -> Value {
    json!({
        "pid": -1, "field": "pid", "index": 7, "terminated": false,
        "name": "Phantom Viewer", "bundle_id": "com.example.phantom-viewer",
        "executable": "PhantomViewer", "launch_time": 1_761_654_361.0,
    })
}

fn without(key: &str) -> Value {
    let mut record = phantom();
    record[key] = Value::Null;
    record
}

/// Records that each lack one alias, each with the text of a lookup by the
/// alias it lacks.
pub(crate) fn incomplete_identities() -> [(Value, &'static str); 3] {
    [
        (without("bundle_id"), "com.example.phantom-viewer"),
        (without("executable"), "PhantomViewer"),
        (without("name"), "Phantom Viewer"),
    ]
}

/// The deadline of a lookup that is expected to run out of time.
pub(crate) fn shortly() -> Instant {
    Instant::now() + Duration::from_secs(1)
}

/// The deadline of a lookup that is expected to finish.
pub(crate) fn soon() -> Instant {
    Instant::now() + Duration::from_secs(10)
}

/// An app inventory lookup ran out of time because `record` kept every read
/// incomplete.
pub(crate) fn assert_app_lookup_waited_for(error: &AdapterError, record: &Value) {
    assert_eq!(error.code, ErrorCode::Timeout, "{error:?}");
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["kind"], "application_inventory_unstable");
    assert_eq!(details["last_failure_details"]["skipped"], json!([record]));
}

/// The scoped inventory finds an app by display name, bundle ID or executable
/// name. A record without the alias it compares keeps it waiting, and so does
/// a record whose alias matches; a record with every alias lets a lookup for
/// another app finish empty.
#[test]
fn the_scoped_inventory_waits_for_a_record_without_the_alias_it_compares() {
    let scoped = crate::system::app::app_inventory::list_apps_scoped_until;
    for (record, text) in incomplete_identities() {
        let _snapshot = install_phantom(&record);
        let error = scoped(text, None, shortly()).unwrap_err();
        assert_app_lookup_waited_for(&error, &record);
    }
    let _snapshot = install_phantom(&phantom());
    let error = scoped("com.example.phantom-viewer", None, shortly()).unwrap_err();
    assert_app_lookup_waited_for(&error, &phantom());
    assert!(scoped(ABSENT, None, soon()).unwrap().is_empty());
}

/// A lookup pinned to a bundle ID, as the re-read before a mutation is, needs
/// every alias of a live record too.
#[test]
fn a_pinned_lookup_waits_for_a_record_without_every_alias() {
    let scoped = crate::system::app::app_inventory::list_apps_scoped_until;
    let pin = Some("com.example.absent");
    for (record, _) in incomplete_identities() {
        let _snapshot = install_phantom(&record);
        let error = scoped(ABSENT, pin, shortly()).unwrap_err();
        assert_app_lookup_waited_for(&error, &record);
    }
    let _snapshot = install_phantom(&phantom());
    assert!(scoped(ABSENT, pin, soon()).unwrap().is_empty());
}

/// A window lookup by app reads the window owners and reports the app as not
/// running only once a live record rules it out.
#[test]
fn a_window_lookup_waits_for_a_record_without_the_alias_it_compares() {
    let windows = |app: &str, deadline: Instant| {
        crate::system::window::window_inventory::list_windows_until(
            &WindowFilter {
                app: Some(app.to_owned()),
                focused_only: false,
            },
            deadline,
        )
    };
    for (record, text) in incomplete_identities() {
        let _snapshot = install_phantom(&record);
        let error = windows(text, shortly()).unwrap_err();
        assert_eq!(error.code, ErrorCode::Timeout, "{error:?}");
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "global_window_inventory_unstable");
        assert_eq!(details["skipped"], json!([record]));
    }
    let _snapshot = install_phantom(&phantom());
    let error = windows(ABSENT, soon()).unwrap_err();
    assert_eq!(error.code, ErrorCode::AppNotFound, "{error:?}");
}
