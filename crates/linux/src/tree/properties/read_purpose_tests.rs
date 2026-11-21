use super::ReadPurpose;

#[test]
fn gate_reads_the_child_list_and_none_of_the_evidence() {
    let purpose = ReadPurpose::Gate;
    assert!(purpose.children());
    assert!(!purpose.identity());
    assert!(!purpose.vocabulary());
    assert!(!purpose.value());
    assert!(!purpose.bounds());
    assert!(!purpose.actions());
}

#[test]
fn shape_leaves_the_child_list_to_the_path_descent() {
    assert!(!ReadPurpose::Shape.children());
}
