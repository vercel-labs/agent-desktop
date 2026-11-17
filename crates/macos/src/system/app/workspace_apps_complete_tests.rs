use super::*;
use agent_desktop_core::ErrorCode;
use serde_json::json;
use std::time::{Duration, Instant};

/// The live record reported on a real Mac: Preview with pid -1 while its
/// process ran under a pid AppKit never bound.
fn preview() -> Value {
    json!({
        "pid": -1, "field": "pid", "index": 94, "terminated": false,
        "name": "Preview", "bundle_id": "com.apple.Preview", "executable": "Preview",
        "launch_time": 1_761_654_361.0,
    })
}

/// Another live record reported the same way: Xcode's Device Hub.
fn device_hub() -> Value {
    json!({
        "pid": -1, "field": "pid", "index": 94, "terminated": false,
        "name": "Device Hub", "bundle_id": "com.apple.dt.Devices", "executable": "DeviceHub",
        "launch_time": 1_761_000_000.0,
    })
}

/// The terminated record reported on another Mac: no name, bundle or launch date.
fn terminated() -> Value {
    json!({"pid": -1, "field": "pid", "index": 67, "terminated": true})
}

fn with(mut record: Value, key: &str, value: Value) -> Value {
    record[key] = value;
    record
}

fn calculator() -> Wanted<'static> {
    Wanted::from("Calculator")
}

fn soon() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn check(skipped: &[Value], previous: &mut Option<Vec<ProcesslessRecord>>) -> bool {
    require_complete_with(skipped, calculator(), previous).is_ok()
}

/// A first sighting is not yet stable; the same record again passes even at
/// another index; an empty read resets it.
#[test]
fn a_processless_record_passes_once_it_repeats_unchanged() {
    let mut previous = None;
    assert!(!check(&[preview()], &mut previous));
    assert!(check(&[with(preview(), "index", json!(95))], &mut previous));
    assert!(check(&[preview()], &mut previous));
    assert!(check(&[], &mut previous));
    assert!(!check(&[preview()], &mut previous));
}

/// One phantom leaving while another arrives with the same pid is churn, and
/// so is any change to what identifies a record or to how many there are.
#[test]
fn a_replaced_or_changed_processless_record_is_churn() {
    let mut previous = None;
    assert!(!check(&[preview()], &mut previous));
    assert!(check(&[preview()], &mut previous));
    assert!(!check(&[device_hub()], &mut previous));
    assert!(check(&[device_hub()], &mut previous));
    for changed in [
        with(preview(), "pid", json!(0)),
        with(preview(), "terminated", json!(true)),
        with(preview(), "name", json!("Vorschau")),
        with(preview(), "bundle_id", json!("com.example.Preview")),
        with(preview(), "executable", json!("PreviewAgent")),
        with(preview(), "launch_time", json!(1_761_654_362.0)),
        with(preview(), "launch_time", Value::Null),
    ] {
        let mut previous = None;
        assert!(!check(&[preview()], &mut previous));
        assert!(check(&[preview()], &mut previous));
        assert!(
            !check(std::slice::from_ref(&changed), &mut previous),
            "{changed}"
        );
    }
    let mut previous = None;
    assert!(!check(&[preview()], &mut previous));
    assert!(check(&[preview()], &mut previous));
    assert!(!check(&[preview(), preview()], &mut previous));
}

/// Without an enclosing operation nothing compares; an operation starts
/// empty, a nested one neither sees nor changes the outer reads, and the reads
/// end with the operation that made them.
#[test]
fn records_from_another_operation_never_count() {
    let wanted = calculator();
    assert!(require_complete(&[preview()], wanted).is_err());
    assert!(require_complete(&[preview()], wanted).is_err());
    inventory_operation(|| {
        assert!(require_complete(&[preview()], wanted).is_err());
        require_complete(&[preview()], wanted).unwrap();
        inventory_operation(|| {
            assert!(require_complete(&[preview()], wanted).is_err());
            assert!(require_complete(&[device_hub()], wanted).is_err());
            require_complete(&[device_hub()], wanted).unwrap();
        });
        require_complete(&[preview()], wanted).unwrap();
    });
    assert!(require_complete(&[preview()], wanted).is_err());
    inventory_operation(|| assert!(require_complete(&[preview()], wanted).is_err()));
}

#[test]
fn an_operation_that_panics_leaves_no_reads_behind() {
    let wanted = calculator();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        inventory_operation(|| {
            assert!(require_complete(&[preview()], wanted).is_err());
            require_complete(&[preview()], wanted).unwrap();
            panic!("inventory read panicked");
        })
    }));
    assert!(result.is_err());
    OPERATION.with(|operation| assert!(operation.borrow().is_none()));
    inventory_operation(|| assert!(require_complete(&[preview()], wanted).is_err()));
}

/// Both retry loops run as one operation: a stable phantom lets them finish,
/// one that changes on every read never does, and nothing outlives them.
#[test]
fn the_retry_loops_compare_only_the_reads_of_their_own_operation() {
    let mut reads = 0_u32;
    let apps = crate::system::app::app_inventory::stabilize_apps_until(soon(), || {
        reads += 1;
        require_complete(&[preview()], calculator())?;
        Ok(Vec::new())
    })
    .unwrap();
    assert!(apps.is_empty());
    assert_eq!(reads, 3);
    let mut flip = false;
    let error = crate::system::app::app_inventory::stabilize_apps_until(
        Instant::now() + Duration::from_millis(200),
        || {
            flip = !flip;
            let record = if flip { preview() } else { device_hub() };
            require_complete(&[record], calculator())?;
            Ok(Vec::new())
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
    let windows =
        crate::system::window::window_inventory_global::stabilize_global_with(soon(), || {
            require_complete(&[preview()], calculator())?;
            Ok(Vec::new())
        })
        .unwrap();
    assert!(windows.is_empty());
    assert!(require_complete(&[preview()], calculator()).is_err());
}

/// A live record never passes for a lookup that could mean its app, or for a
/// caller that wants every app, however long it stays.
#[test]
fn a_live_processless_record_never_passes_for_the_app_it_could_be() {
    let renamed = with(preview(), "executable", json!("PreviewAgent"));
    let unbundled = with(preview(), "bundle_id", Value::Null);
    for (record, wanted) in [
        (preview(), Wanted::from("Preview")),
        (preview(), Wanted::from("preview")),
        (preview(), Wanted::from("com.apple.preview")),
        (preview(), Wanted::from("/System/Applications/Preview.app")),
        (preview(), Wanted::from("Preview.app")),
        (renamed, Wanted::from("PreviewAgent")),
        (
            preview(),
            Wanted::App {
                name: "Viewer",
                bundle_id: Some("com.apple.Preview"),
            },
        ),
        (
            unbundled,
            Wanted::App {
                name: "Calculator",
                bundle_id: Some("com.apple.calculator"),
            },
        ),
        (preview(), Wanted::Any),
    ] {
        let mut previous = None;
        for _ in 0..3 {
            assert!(
                require_complete_with(std::slice::from_ref(&record), wanted, &mut previous)
                    .is_err(),
                "{wanted:?}"
            );
        }
    }
}

/// A record that lacks its display name, bundle ID or executable name, or
/// carries a blank one, may lack the one a lookup matches. It never passes for
/// a lookup by that very alias, for a lookup for another app, or for a lookup
/// pinned to a bundle ID. The text of a lookup is compared with every field,
/// because a display name can look like a bundle ID.
#[test]
fn a_live_record_without_every_alias_never_passes_for_a_lookup() {
    let pinned = Wanted::App {
        name: "Calculator",
        bundle_id: Some("com.apple.calculator"),
    };
    let mut cases = Vec::new();
    for (key, alias) in [
        ("bundle_id", "com.apple.dt.Devices"),
        ("executable", "DeviceHub"),
        ("name", "Device Hub"),
    ] {
        for missing in [Value::Null, json!(" ")] {
            let record = with(device_hub(), key, missing);
            cases.push((record.clone(), Wanted::from(alias)));
            cases.push((record.clone(), calculator()));
            cases.push((record, pinned));
        }
    }
    let dotted = with(device_hub(), "name", json!("com.example.Hub"));
    cases.push((dotted, Wanted::from("com.example.hub")));
    for (record, wanted) in cases {
        let mut previous = None;
        for _ in 0..3 {
            assert!(
                require_complete_with(std::slice::from_ref(&record), wanted, &mut previous)
                    .is_err(),
                "{record} {wanted:?}"
            );
        }
    }
}

/// A record with every alias passes once stable for a lookup that none of its
/// aliases matches, pinned to a bundle ID or not.
#[test]
fn a_live_record_with_every_alias_passes_for_another_app() {
    for wanted in [
        calculator(),
        Wanted::from("com.apple.calculator"),
        Wanted::App {
            name: "Calculator",
            bundle_id: Some("com.apple.calculator"),
        },
    ] {
        let mut previous = None;
        assert!(require_complete_with(&[device_hub()], wanted, &mut previous).is_err());
        require_complete_with(&[device_hub()], wanted, &mut previous).unwrap();
    }
}

/// A terminated record belongs to no process, so it passes once stable, even
/// without any identity and for a caller that wants every app.
#[test]
fn a_terminated_processless_record_passes_once_stable_for_every_caller() {
    for wanted in [Wanted::Any, Wanted::from("Preview"), calculator()] {
        let mut previous = None;
        assert!(require_complete_with(&[terminated()], wanted, &mut previous).is_err());
        require_complete_with(
            &[with(terminated(), "index", json!(70))],
            wanted,
            &mut previous,
        )
        .unwrap();
    }
}

#[test]
fn other_skipped_records_keep_the_inventory_incomplete() {
    let mut without_flag = preview();
    without_flag.as_object_mut().unwrap().remove("terminated");
    let name_only = with(
        with(preview(), "bundle_id", Value::Null),
        "executable",
        Value::Null,
    );
    for skipped in [
        vec![json!({"pid": 11, "field": "activation_policy"})],
        vec![json!({"pid": -1, "field": "pid"})],
        vec![without_flag],
        vec![name_only],
        vec![name_only_blank()],
        vec![with(preview(), "pid", json!("-1"))],
        vec![with(preview(), "pid", json!(11))],
        vec![with(preview(), "name", json!(42))],
        vec![with(preview(), "launch_time", json!(-1.0))],
        vec![with(preview(), "terminated", json!("false"))],
        vec![with(terminated(), "field", json!("frontmost_pid"))],
        vec![preview(), json!({"pid": 12, "field": "duplicate_pid"})],
    ] {
        let mut previous = None;
        for _ in 0..3 {
            assert!(!check(&skipped, &mut previous), "{skipped:?}");
        }
    }
}

fn name_only_blank() -> Value {
    with(
        with(preview(), "bundle_id", json!(" ")),
        "executable",
        json!(""),
    )
}

/// A read with a record that cannot pass breaks the chain of stable reads.
#[test]
fn a_record_that_cannot_pass_restarts_stability() {
    let mut previous = None;
    assert!(!check(&[preview()], &mut previous));
    assert!(check(&[preview()], &mut previous));
    assert!(!check(
        &[preview(), json!({"pid": 11, "field": "activation_policy"})],
        &mut previous
    ));
    assert!(!check(&[preview()], &mut previous));
    assert!(check(&[preview()], &mut previous));
}

/// The bridge's snapshot keeps the record's identity in `skipped`, and both
/// inventories accept it for another app once it repeats in one operation.
#[test]
fn a_phantom_appkit_record_blocks_only_a_lookup_that_could_mean_it() {
    let bytes = serde_json::to_vec(&json!({
        "applications": [
            {"name":"Finder", "pid":10, "launch_time":100.0, "activation_policy":"regular"},
        ],
        "frontmost_pid": 10, "frontmost_launch_time": 100.0,
        "skipped": [preview()],
    }))
    .unwrap();
    let read = |name: &'static str| {
        super::super::apps_inventory_from_json_with(
            &bytes,
            soon(),
            move |app| app.name == name,
            false,
            |pid| Ok(Some(format!("instance-{pid}"))),
        )
        .unwrap()
    };
    let first = read("Calculator");
    assert!(first.apps.is_empty());
    assert_eq!(first.skipped, vec![preview()]);
    inventory_operation(|| {
        assert!(require_complete(&read("Calculator").skipped, calculator()).is_err());
        require_complete(&read("Calculator").skipped, calculator()).unwrap();
        for _ in 0..3 {
            let skipped = read("Preview").skipped;
            assert!(require_complete(&skipped, Wanted::from("Preview")).is_err());
        }
    });
    let owners = super::super::window_owner_snapshot_from_json(&bytes, soon()).unwrap();
    assert_eq!(owners.frontmost().unwrap().pid, 10);
    inventory_operation(|| {
        assert!(owners.require_match("Calculator").is_err());
        owners.require_match("Calculator").unwrap();
        owners.require_match("Finder").unwrap();
        assert!(owners.require_match("Preview").is_err());
        assert!(owners.require_match("Preview").is_err());
    });
    assert!(owners.require_match("Calculator").is_err());
}
