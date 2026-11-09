use super::{parse_finite_f64, without_terminal_paragraph};

#[test]
fn parse_finite_rejects_nan_and_non_numeric() {
    assert_eq!(parse_finite_f64("77"), Some(77.0));
    assert!(parse_finite_f64("abc").is_none());
    assert!(parse_finite_f64("NaN").is_none());
    assert!(parse_finite_f64("inf").is_none());
}

#[test]
fn a_rich_edit_terminal_paragraph_mark_is_not_part_of_the_read_value() {
    assert_eq!(without_terminal_paragraph("abc\r".into(), "abc"), "abc");
    assert_eq!(without_terminal_paragraph("\r".into(), ""), "");
    assert_eq!(without_terminal_paragraph("abc\r".into(), "abc\r"), "abc\r");
    assert_eq!(without_terminal_paragraph("abc".into(), "abc"), "abc");
}
