use super::scope_tests::{ScopeAdapter, landed_path, resolve as resolve_scoped};
use super::test_support::{RuleAdapter, capture_by, node, resolve, tree_with};
use crate::refs::refs_test_support::HomeGuard;
use crate::{AccessibilityNode, ElementIdentifier, ErrorCode, IdentifierKind};

fn labelled_field(label: &str, id: Option<&str>) -> AccessibilityNode {
    let mut field = node("textfield", label);
    field.identity.native_id = id.map(|value| ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: value.into(),
    });
    field.identity.name_is_borrowed = true;
    field
}

fn the_field(entry: &crate::RefEntry) -> bool {
    entry.identity.role == "textfield"
}

#[test]
fn a_field_named_by_a_label_keeps_its_ref_when_the_label_changes() {
    let _guard = HomeGuard::new();
    let entry = capture_by(
        tree_with(vec![labelled_field("Count 0", Some("txtName"))]),
        the_field,
    );
    assert_eq!(entry.identity.name, None);

    let adapter = RuleAdapter::new(tree_with(vec![labelled_field("Count 1", Some("txtName"))]));
    assert!(resolve(&adapter, &entry).is_ok());
}

#[test]
fn a_borrowed_name_stays_identity_when_no_native_id_is_recorded() {
    let _guard = HomeGuard::new();
    let entry = capture_by(tree_with(vec![labelled_field("Count 0", None)]), the_field);
    assert_eq!(entry.identity.name.as_deref(), Some("Count 0"));

    let adapter = RuleAdapter::new(tree_with(vec![labelled_field("Count 1", None)]));
    let Err(error) = resolve(&adapter, &entry) else {
        panic!("the field is no longer proven");
    };
    assert_eq!(error.code, ErrorCode::StaleRef);
}

fn labelled_row(label: &str, id: &str) -> AccessibilityNode {
    let mut row = node("group", label);
    row.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: id.into(),
    });
    row.identity.name_is_borrowed = true;
    row.children = vec![node("button", "Name")];
    row
}

#[test]
fn a_cell_scoped_by_a_labelled_row_keeps_its_ref_when_the_label_changes() {
    let _guard = HomeGuard::new();
    let rows = |label| {
        tree_with(vec![
            labelled_row("Row 0", "row0"),
            labelled_row(label, "row1"),
        ])
    };
    let entry = capture_by(rows("Row 1"), |entry| {
        entry.identity.role == "button" && entry.scope.path.first() == Some(&1)
    });
    assert_eq!(entry.scope.chain.len(), 1);
    assert_eq!(entry.scope.chain[0].name, None);

    let adapter = ScopeAdapter::new(rows("Renamed"));
    let handle = resolve_scoped(&adapter, &entry).expect("the row still matches by its id");
    assert_eq!(landed_path(&handle), [1, 0]);
}
