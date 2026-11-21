use super::ElementIdentifier;
use crate::IdentifierKind;

fn address(row_index: u32, column_index: u32) -> String {
    ElementIdentifier::grid_cell(row_index, column_index).value
}

#[test]
fn a_grid_cell_is_addressed_by_column_letters_and_a_row_counted_from_one() {
    let cell = ElementIdentifier::grid_cell(6, 1);

    assert_eq!(cell.kind, IdentifierKind::GridCell);
    assert_eq!(cell.value, "B7");
    assert_eq!(address(0, 0), "A1");
}

#[test]
fn column_letters_roll_over_after_z() {
    assert_eq!(address(0, 25), "Z1");
    assert_eq!(address(0, 26), "AA1");
    assert_eq!(address(0, 27), "AB1");
    assert_eq!(address(0, 701), "ZZ1");
    assert_eq!(address(0, 702), "AAA1");
    assert_eq!(address(1_048_575, 16_383), "XFD1048576");
}

#[test]
fn the_largest_indexes_do_not_overflow() {
    assert_eq!(address(u32::MAX, 0), "A4294967296");
    assert!(!address(0, u32::MAX).is_empty());
}

#[test]
fn the_kind_is_written_in_snake_case() {
    let json = serde_json::to_string(&ElementIdentifier::grid_cell(2, 2)).unwrap();

    assert_eq!(json, r#"{"kind":"grid_cell","value":"C3"}"#);
}
