use super::{ReadBack, classify};

#[test]
fn the_read_back_judges_only_an_empty_cell_as_a_miss() {
    assert_eq!(classify(Some("42"), "42"), ReadBack::Matches);
    assert_eq!(classify(None, ""), ReadBack::Matches);
    assert_eq!(classify(Some(""), ""), ReadBack::Matches);
    assert_eq!(classify(None, "42"), ReadBack::Empty);
    assert_eq!(classify(Some(""), "42"), ReadBack::Empty);
    assert_eq!(classify(Some("41"), "42"), ReadBack::Differs);
    assert_eq!(classify(Some("42"), ""), ReadBack::Differs);
}
