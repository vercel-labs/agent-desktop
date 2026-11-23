use super::*;

#[test]
fn an_absent_extension_and_an_incompatible_one_name_different_causes() {
    assert_eq!(Unavailable::Absent.cause(), "shell_extension_absent");
    assert_eq!(
        Unavailable::Incompatible.cause(),
        "shell_extension_incompatible"
    );
}

#[test]
fn both_suggestions_name_the_extension_and_the_new_login_it_needs() {
    for unavailable in [Unavailable::Absent, Unavailable::Incompatible] {
        let suggestion = unavailable.suggestion();
        assert!(suggestion.contains(EXTENSION_UUID), "{suggestion}");
        assert!(suggestion.contains("log out and back in"), "{suggestion}");
    }
}

#[test]
fn the_uuid_matches_the_extension_metadata_shipped_beside_the_crate() {
    let metadata = include_str!(
        "../../../../gnome-shell-extension/agent-desktop@agent-browser.dev/metadata.json"
    );
    let metadata: serde_json::Value = serde_json::from_str(metadata).expect("metadata is JSON");
    assert_eq!(metadata["uuid"], EXTENSION_UUID);
}

#[test]
fn the_bus_name_path_interface_and_version_match_the_extension_source() {
    let source = include_str!(
        "../../../../gnome-shell-extension/agent-desktop@agent-browser.dev/extension.js"
    );
    assert!(source.contains(&format!("const BUS_NAME = '{BUS_NAME}';")));
    assert!(source.contains(&format!("const OBJECT_PATH = '{OBJECT_PATH}';")));
    assert!(source.contains(&format!("<interface name=\"{INTERFACE}\">")));
    assert!(source.contains(&format!("const PROTOCOL_VERSION = {PROTOCOL_VERSION};")));
}

#[test]
fn the_extension_holds_only_files_gnome_extensions_pack_includes_by_default() {
    let packed = [
        "metadata.json",
        "extension.js",
        "prefs.js",
        "stylesheet.css",
    ];
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/gnome-shell-extension/agent-desktop@agent-browser.dev"
    );
    for entry in std::fs::read_dir(directory).expect("extension directory") {
        let name = entry.expect("entry").file_name();
        let name = name.to_string_lossy();
        assert!(
            packed.contains(&name.as_ref()),
            "{name} is left out of `gnome-extensions pack` without --extra-source"
        );
    }
}

#[test]
fn the_extension_draws_with_the_label_and_timing_numbers_core_shares() {
    let source = include_str!(
        "../../../../gnome-shell-extension/agent-desktop@agent-browser.dev/extension.js"
    );
    let numbers = [
        ("LABEL_FONT_POINTS", agent_desktop_core::LABEL_FONT_POINTS),
        ("LABEL_PILL_HEIGHT", agent_desktop_core::LABEL_PILL_HEIGHT),
        ("LABEL_PILL_PADDING", agent_desktop_core::LABEL_PILL_PADDING),
        (
            "LABEL_PILL_MAX_WIDTH",
            agent_desktop_core::LABEL_PILL_MAX_WIDTH,
        ),
        (
            "LABEL_SHADOW_OPACITY",
            agent_desktop_core::LABEL_SHADOW_OPACITY,
        ),
        (
            "LABEL_SHADOW_OFFSET",
            agent_desktop_core::LABEL_SHADOW_OFFSET,
        ),
        ("LABEL_SHADOW_BLUR", agent_desktop_core::LABEL_SHADOW_BLUR),
        (
            "LABEL_REVEAL_MS",
            agent_desktop_core::CURSOR_LABEL_REVEAL_MS as f64,
        ),
        (
            "IDLE_REST_MS",
            agent_desktop_core::CURSOR_IDLE_REST_MS as f64,
        ),
        (
            "REST_FADE_MS",
            agent_desktop_core::CURSOR_REST_FADE_MS as f64,
        ),
    ];
    for (name, value) in numbers {
        let line = source
            .lines()
            .find(|line| line.starts_with(&format!("const {name} = ")))
            .unwrap_or_else(|| panic!("extension.js declares no {name}"));
        let declared: f64 = line
            .trim_start_matches(&format!("const {name} = "))
            .trim_end_matches(';')
            .parse()
            .unwrap_or_else(|_| panic!("{line} is not a number"));
        assert_eq!(declared, value, "{name}");
    }
}
