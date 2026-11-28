use super::*;
use agent_desktop_core::IdentifierKind;

#[test]
fn zero_based_indexes_become_a_spreadsheet_address() {
    let identifier = cell_identifier(Some(6), Some(1)).expect("identifier");

    assert_eq!(identifier.kind, IdentifierKind::GridCell);
    assert_eq!(identifier.value, "B7");
}

#[test]
fn a_cell_without_both_indexes_gets_no_identifier() {
    assert!(cell_identifier(Some(2), None).is_none());
    assert!(cell_identifier(None, Some(2)).is_none());
    assert!(cell_identifier(Some(-1), Some(0)).is_none());
}

#[test]
fn table_size_keeps_each_count_the_table_answers() {
    let both = table_size(Some(22), Some(7));
    assert_eq!((both.row_count, both.column_count), (Some(22), Some(7)));

    let rows_only = table_size(Some(5), None);
    assert_eq!(
        (rows_only.row_count, rows_only.column_count),
        (Some(5), None)
    );

    assert_eq!(table_size(None, None), NodeDescriptor::default());
    assert_eq!(table_size(Some(-3), None), NodeDescriptor::default());
}

#[test]
fn only_cell_and_table_roles_pay_for_the_extra_reads() {
    assert!(is_cell_role(Some("AXCell")));
    assert!(!is_cell_role(Some("AXButton")));
    assert!(is_table_role(Some("AXTable")));
    assert!(is_table_role(Some("AXOutline")));
    assert!(!is_table_role(Some("AXRow")));
    assert!(!is_table_role(None));
}

#[test]
fn a_cell_that_spans_several_positions_gets_no_address() {
    assert_eq!(single_index(3, 1), Some(3));
    assert_eq!(single_index(0, 4), None);
    assert_eq!(single_index(0, 0), None);
}

#[test]
fn a_cell_without_its_own_token_gets_no_address() {
    assert!(takes_address(Some("AXCell"), true));
    assert!(!takes_address(Some("AXCell"), false));
    assert!(!takes_address(Some("AXButton"), true));
}
