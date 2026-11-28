use super::*;

fn server() -> Server {
    Server {
        tools: catalog::tools(),
        compact: false,
        screenshot_dir: None,
        globals: Vec::new(),
        calls: calls::Calls::default(),
    }
}

fn compact_server() -> Server {
    Server {
        tools: catalog::tools_with_options(true),
        compact: true,
        screenshot_dir: Some(PathBuf::from("/tmp/agy-screens")),
        globals: Vec::new(),
        calls: calls::Calls::default(),
    }
}

#[test]
fn legacy_initialize_echoes_a_supported_version_and_falls_back_to_the_newest() {
    let server = server();
    let echoed = server
        .handle(
            &Value::Null,
            "initialize",
            &json!({ "protocolVersion": "2025-06-18" }),
        )
        .unwrap_or_else(|_| panic!("initialize"));
    assert_eq!(echoed["protocolVersion"], "2025-06-18");
    let fallback = server
        .handle(
            &Value::Null,
            "initialize",
            &json!({ "protocolVersion": "1999-01-01" }),
        )
        .unwrap_or_else(|_| panic!("initialize"));
    assert_eq!(fallback["protocolVersion"], LEGACY_VERSIONS[0]);
    assert_eq!(fallback["capabilities"], json!({ "tools": {} }));
}

#[test]
fn modern_requests_with_an_unknown_version_get_the_supported_list() {
    let params = json!({ "_meta": { PROTOCOL_VERSION_META: "1999-01-01" } });
    let Err(error) = server().handle(&Value::Null, "tools/list", &params) else {
        panic!("an unknown modern version must be refused");
    };
    assert_eq!(error.code, -32022);
    assert_eq!(error.data.unwrap()["supported"][0], MODERN_VERSION);
}

#[test]
fn modern_tools_list_is_cacheable_and_complete() {
    let params = json!({ "_meta": { PROTOCOL_VERSION_META: MODERN_VERSION } });
    let result = server()
        .handle(&Value::Null, "tools/list", &params)
        .unwrap_or_else(|_| panic!("tools/list"));
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["cacheScope"], "public");
    assert_eq!(
        result["tools"].as_array().unwrap().len(),
        catalog::tools().len()
    );
}

#[test]
fn compact_tools_list_exposes_four_grouped_tools_and_matching_instructions() {
    let srv = compact_server();
    let result = srv
        .handle(&Value::Null, "tools/list", &json!({}))
        .unwrap_or_else(|_| panic!("compact tools/list"));
    let tools = result["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);
    let names = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
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
    let mentioned = srv
        .instructions()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with("desktop_"))
        .collect::<Vec<_>>();
    assert!(!mentioned.is_empty());
    for word in mentioned {
        assert!(names.contains(&word), "compact instructions name {word}");
    }
}

#[test]
fn default_screenshot_dir_populates_missing_output_path() {
    let leaf = catalog::tools()
        .into_iter()
        .find(|t| t.path == ["screenshot"])
        .expect("screenshot tool");
    let mut args = Map::new();
    let dir = PathBuf::from("/tmp/agy-screens");
    compact::apply_default_screenshot_path(&leaf, &mut args, Some(&dir));
    let out = args
        .get("output_path")
        .and_then(Value::as_str)
        .expect("output_path populated");
    let out = PathBuf::from(out);
    assert!(out.starts_with(&dir), "{}", out.display());
    let file_name = out
        .file_name()
        .and_then(|name| name.to_str())
        .expect("screenshot file name");
    assert!(file_name.starts_with("screenshot-"), "{file_name}");
    assert!(file_name.ends_with(".png"), "{file_name}");

    let mut second_args = Map::new();
    compact::apply_default_screenshot_path(&leaf, &mut second_args, Some(&dir));
    let second = second_args
        .get("output_path")
        .and_then(Value::as_str)
        .expect("second output_path");
    assert_ne!(
        out,
        PathBuf::from(second),
        "back-to-back screenshots must get distinct paths"
    );

    let mut unset_args = Map::new();
    compact::apply_default_screenshot_path(&leaf, &mut unset_args, None);
    assert!(unset_args.get("output_path").is_none());
}

#[test]
fn compact_server_refuses_unlisted_direct_leaf_tools() {
    let Err(error) = compact_server().handle(
        &Value::Null,
        "tools/call",
        &json!({ "name": "desktop_sandbox_exec", "arguments": { "name": "s", "args": [] } }),
    ) else {
        panic!("compact mode must not dispatch unlisted leaf tools");
    };
    assert_eq!(error.code, -32602);
}

#[test]
fn unknown_tools_are_protocol_errors() {
    let Err(error) = server().handle(
        &Value::Null,
        "tools/call",
        &json!({ "name": "desktop_nope" }),
    ) else {
        panic!("unknown tool must be refused");
    };
    assert_eq!(error.code, -32602);
}

#[test]
fn instructions_name_only_tools_that_exist() {
    let names = catalog::tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    let mentioned = INSTRUCTIONS
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with("desktop_"))
        .collect::<Vec<_>>();
    assert!(!mentioned.is_empty());
    for word in mentioned {
        assert!(
            names.iter().any(|name| name == word),
            "instructions name {word}"
        );
    }
}

#[test]
fn inline_screenshots_become_image_content() {
    let envelope = json!({ "ok": true, "command": "screenshot", "data": { "data": "iVBORw0K", "format": "png", "width": 1 } });
    let result = call::envelope_result(envelope);
    assert_eq!(result["isError"], false);
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(result["content"][1]["mimeType"], "image/png");
    assert!(result["structuredContent"]["data"].get("data").is_none());
    let failed = call::envelope_result(json!({ "ok": false, "command": "click" }));
    assert_eq!(failed["isError"], true);
}

#[test]
fn oversized_messages_are_refused_and_the_next_message_still_reads() {
    let mut input = Vec::new();
    input.extend(std::iter::repeat_n(b'x', MAX_MESSAGE_BYTES + 10));
    input.extend(b"\n\n{\"id\":1}\n");
    let mut reader = std::io::Cursor::new(input);
    let Some(Err(error)) = read_message(&mut reader) else {
        panic!("an oversized message must be refused");
    };
    assert_eq!(error.code, -32600);
    let Some(Ok(next)) = read_message(&mut reader) else {
        panic!("the following message must still be read");
    };
    assert_eq!(next, b"{\"id\":1}\n");
    assert!(read_message(&mut reader).is_none());
}
