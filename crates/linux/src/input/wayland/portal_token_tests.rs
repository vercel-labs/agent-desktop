use super::{next_handle_token, request_path};

#[test]
fn request_path_replaces_leading_colon_and_dots() {
    assert_eq!(
        request_path(":1.42", "tok_1"),
        "/org/freedesktop/portal/desktop/request/1_42/tok_1"
    );
}

#[test]
fn handle_tokens_are_unique_and_valid_dbus_segments() {
    let first = next_handle_token("rd");
    let second = next_handle_token("rd");
    assert_ne!(first, second);
    assert!(
        first
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    );
}
