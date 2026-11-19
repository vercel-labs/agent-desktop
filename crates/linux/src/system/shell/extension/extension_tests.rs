use super::*;

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
