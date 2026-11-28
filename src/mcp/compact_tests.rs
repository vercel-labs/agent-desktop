use super::*;
use crate::mcp::catalog;

#[test]
fn compact_surface_exposes_four_tools_with_valid_schemas() {
    let leaf = catalog::tools_with_options(true);
    let defs = definitions(&leaf);
    let names = defs
        .iter()
        .filter_map(|d| d["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "desktop_observe",
            "desktop_act",
            "desktop_window",
            "desktop_batch"
        ]
    );
    for def in &defs {
        assert_eq!(def["inputSchema"]["type"], "object");
        assert_eq!(def["inputSchema"]["additionalProperties"], false);
    }
}

#[test]
fn compact_resolve_accepts_both_flat_and_nested_arguments() {
    let leaf = catalog::tools_with_options(true);
    let flat = json!({
        "command": "snapshot",
        "app": "Finder",
        "interactive_only": true,
        "wait_for": "button:OK"
    });
    let (tool, args) = resolve_call("desktop_observe", flat.as_object().unwrap(), &leaf)
        .expect("valid")
        .expect("matched");
    assert_eq!(tool.name, "desktop_snapshot");
    let argv = tool.argv(&args).expect("argv builds");
    assert_eq!(
        argv,
        [
            "--wait-for=button:OK",
            "snapshot",
            "--app=Finder",
            "--interactive-only"
        ]
    );

    let nested = json!({
        "command": "double_click",
        "args": {
            "ref_id": "@s8f3k2p9:e1",
            "headed": true
        }
    });
    let (tool2, args2) = resolve_call("desktop_act", nested.as_object().unwrap(), &leaf)
        .expect("valid")
        .expect("matched");
    assert_eq!(tool2.name, "desktop_double_click");
    let argv2 = tool2.argv(&args2).expect("argv2 builds");
    assert_eq!(argv2, ["--headed", "double-click", "--", "@s8f3k2p9:e1"]);

    let flat_launch = json!({
        "command": "launch",
        "app": "Calculator",
        "args": ["--flag", "val"]
    });
    let (tool3, args3) = resolve_call("desktop_window", flat_launch.as_object().unwrap(), &leaf)
        .expect("valid")
        .expect("matched");
    assert_eq!(tool3.name, "desktop_launch");
    let argv3 = tool3.argv(&args3).expect("argv3 builds");
    assert_eq!(
        argv3,
        ["launch", "--arg=--flag", "--arg=val", "--", "Calculator"]
    );
}

#[test]
fn compact_resolve_rejects_command_in_wrong_group() {
    let leaf = catalog::tools_with_options(false);
    let bad = json!({ "command": "click", "ref_id": "@s8f3k2p9:e1" });
    let err = resolve_call("desktop_observe", bad.as_object().unwrap(), &leaf).unwrap_err();
    assert!(err.contains("not valid for desktop_observe"), "{err}");
}

#[test]
fn desktop_observe_strips_and_rejects_mutating_arguments() {
    let leaf = catalog::tools_with_options(true);
    let defs = definitions(&leaf);
    let observe_props = defs[0]["inputSchema"]["properties"].as_object().unwrap();
    for forbidden in ["output_path", "request", "guide", "headed"] {
        assert!(
            !observe_props.contains_key(forbidden),
            "desktop_observe schema must not advertise '{forbidden}'"
        );
    }

    let write_attempt = json!({
        "command": "screenshot",
        "output_path": "/tmp/overwrite-target.png"
    });
    assert!(
        resolve_call("desktop_observe", write_attempt.as_object().unwrap(), &leaf)
            .unwrap_err()
            .contains("read-only")
    );

    let prompt_attempt = json!({
        "command": "permissions",
        "request": true
    });
    assert!(
        resolve_call(
            "desktop_observe",
            prompt_attempt.as_object().unwrap(),
            &leaf
        )
        .unwrap_err()
        .contains("read-only")
    );

    let guide_attempt = json!({
        "command": "permissions",
        "guide": true
    });
    assert!(
        resolve_call("desktop_observe", guide_attempt.as_object().unwrap(), &leaf)
            .unwrap_err()
            .contains("read-only")
    );

    let headed_attempt = json!({
        "command": "snapshot",
        "headed": true
    });
    assert!(
        resolve_call(
            "desktop_observe",
            headed_attempt.as_object().unwrap(),
            &leaf
        )
        .unwrap_err()
        .contains("read-only")
    );
}

#[test]
fn compact_rejects_duplicate_top_level_and_nested_argument() {
    let leaf = catalog::tools_with_options(false);
    let dup = json!({
        "command": "snapshot",
        "app": "Finder",
        "args": { "app": "Terminal" }
    });
    let err = resolve_call("desktop_observe", dup.as_object().unwrap(), &leaf).unwrap_err();
    assert!(err.contains("both at top level and inside 'args'"), "{err}");
}
