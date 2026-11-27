use super::*;

#[test]
fn a_plain_absolute_value_is_the_executable() {
    assert_eq!(
        parse_registered_path(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        Some(PathBuf::from(
            r"C:\Program Files\Google\Chrome\Application\chrome.exe"
        ))
    );
}

#[test]
fn quotes_and_surrounding_whitespace_are_stripped() {
    assert_eq!(
        parse_registered_path("  \"C:\\Tools\\app.exe\"  "),
        Some(PathBuf::from(r"C:\Tools\app.exe"))
    );
}

#[test]
fn relative_and_empty_values_are_rejected() {
    for value in ["", "   ", "\"\"", "app.exe", r"Tools\app.exe", r"C:app.exe"] {
        assert_eq!(parse_registered_path(value), None, "{value:?}");
    }
}

#[test]
fn an_unregistered_image_resolves_to_nothing() {
    let image = format!("agent-desktop-unregistered-{}.exe", std::process::id());
    assert_eq!(registered_app_path(&image), None);
}

#[cfg(target_os = "windows")]
#[test]
fn a_registered_image_resolves_to_an_existing_file() {
    let Some(path) = ["chrome.exe", "msedge.exe", "wordpad.exe", "iexplore.exe"]
        .into_iter()
        .find_map(registered_app_path)
    else {
        return;
    };
    assert!(path.is_absolute(), "{path:?}");
    assert!(path.is_file(), "{path:?}");
}
