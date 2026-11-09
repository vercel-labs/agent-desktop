use super::issues_tokens;
use crate::tree::properties::toolkit_trust::ToolkitKind;

#[test]
fn only_gtk_refs_carry_a_token() {
    assert!(issues_tokens(ToolkitKind::Gtk3));
    assert!(issues_tokens(ToolkitKind::Gtk4));
}

#[test]
fn chromium_qt_and_unknown_toolkits_keep_the_capture_proof() {
    assert!(!issues_tokens(ToolkitKind::Chromium));
    assert!(!issues_tokens(ToolkitKind::Qt));
    assert!(!issues_tokens(ToolkitKind::Other));
}
