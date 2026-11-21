use super::resolve_vocabulary;
use crate::tree::LabelOutcome;
use crate::tree::test_support::helpers::read_of;
use crate::tree::vocabulary::role_numbers::{PANEL, TABLE};

#[test]
fn a_table_carries_its_row_and_column_counts() {
    let mut read = read_of(TABLE, "", &[]);
    read.node.properties.grid.size = Some((1_048_576, 16_384));
    let vocabulary = resolve_vocabulary(&read.node, &LabelOutcome::Failed, false);
    assert_eq!(vocabulary.descriptor.row_count, Some(1_048_576));
    assert_eq!(vocabulary.descriptor.column_count, Some(16_384));
}

#[test]
fn a_node_without_table_size_is_unchanged() {
    let read = read_of(PANEL, "", &[]);
    let vocabulary = resolve_vocabulary(&read.node, &LabelOutcome::Failed, false);
    assert!(vocabulary.identifiers.identifiers().is_empty());
    assert_eq!(vocabulary.descriptor.row_count, None);
    assert_eq!(vocabulary.descriptor.column_count, None);
}
