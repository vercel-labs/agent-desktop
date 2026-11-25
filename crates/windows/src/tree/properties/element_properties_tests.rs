use super::*;

/// Code review found the `IsPassword` divergence `withholds_content` guards
/// against - a provider answering a documented-`VT_BOOL` property as
/// `VT_I4(1)` - reaching every other gate too, because `gate_open` read only
/// `flag()`. Both halves of the fix are pinned in one test: a nonzero
/// `Known` number opens the gate `withholds_content` already accepts, and a
/// gate nothing read still does not open - without the second assertion a
/// fix that opened every unread gate unconditionally would also pass.
#[test]
fn a_nonzero_known_number_opens_the_gate_but_an_unread_gate_still_does_not() {
    let opened_by_number = ElementProperties::from_reads(vec![
        (
            TreeProperty::ToggleAvailable,
            PropertyOutcome::Known(PropertyValue::Number(1)),
        ),
        (
            TreeProperty::ToggleState,
            PropertyOutcome::Known(PropertyValue::Number(1)),
        ),
    ]);
    assert_eq!(
        opened_by_number.gated_number(TreeProperty::ToggleState),
        Some(1),
        "a provider answering IsTogglePatternAvailable as VT_I4(1) must open the gate, \
         the same way withholds_content already accepts a nonzero VT_I4 IsPassword"
    );

    let gate_never_read = ElementProperties::from_reads(vec![(
        TreeProperty::ToggleState,
        PropertyOutcome::Known(PropertyValue::Number(1)),
    )]);
    assert_eq!(
        gate_never_read.gated_number(TreeProperty::ToggleState),
        None,
        "a gate nothing read must stay closed - reading an unread gate as open \
         would be a worse bug than the one being fixed"
    );
}

/// The same divergence reaches an `…Available` property read directly
/// through `is_true`, not only a state property read through its gate: the
/// `…Available` properties are themselves ungated, so `gated_flag`'s own
/// terminal read - not `gate_open` - is what a caller like
/// `resolve_actions`/`resolve_role` depends on here.
#[test]
fn is_true_reads_a_nonzero_known_number_on_an_ungated_available_property() {
    let properties = ElementProperties::from_reads(vec![(
        TreeProperty::ToggleAvailable,
        PropertyOutcome::Known(PropertyValue::Number(1)),
    )]);

    assert!(
        properties.is_true(TreeProperty::ToggleAvailable),
        "IsTogglePatternAvailable answered as VT_I4(1) must be true, the same way \
         withholds_content already accepts a nonzero VT_I4 IsPassword"
    );
}

fn value_read(class: &str, value: &str) -> LocatorField<String> {
    ElementProperties::from_reads(vec![
        (
            TreeProperty::ClassName,
            PropertyOutcome::Known(PropertyValue::Text(class.into())),
        ),
        (
            TreeProperty::Value,
            PropertyOutcome::Known(PropertyValue::Text(value.into())),
        ),
    ])
    .locator_evidence(ResolvedVocabulary::unknown())
    .value
}

/// Character Map's emptied RichEdit read back `"\r"`, so `get` reported a
/// cleared field as non-empty. Only the one final mark of a RichEdit goes;
/// a plain edit's value and a RichEdit's inner breaks stay verbatim.
#[test]
fn a_rich_edit_value_drops_only_its_final_paragraph_mark() {
    assert_eq!(
        value_read("RICHEDIT50W", "\r"),
        LocatorField::Known(String::new())
    );
    assert_eq!(
        value_read("RichEdit50W", "a\rb\r"),
        LocatorField::Known("a\rb".into())
    );
    assert_eq!(
        value_read("Edit", "hi\r"),
        LocatorField::Known("hi\r".into())
    );
    assert_eq!(
        value_read("RichEdit20W", "hi"),
        LocatorField::Known("hi".into())
    );
}

fn read_through_the_bound(class: &str, raw: String) -> LocatorField<String> {
    ElementProperties::from_reads(vec![
        (
            TreeProperty::ClassName,
            PropertyOutcome::Known(PropertyValue::Text(class.into())),
        ),
        (
            TreeProperty::Value,
            crate::tree::properties::bounded_property_text(TreeProperty::Value, raw),
        ),
    ])
    .locator_evidence(ResolvedVocabulary::unknown())
    .value
}

/// A RichEdit field of exactly the evidence bound reads back the bound plus its
/// paragraph mark; counting the mark made `get`, `clear` and `set-value` fail
/// on it with incomplete evidence.
#[test]
fn a_rich_edit_value_is_bounded_by_its_content_not_its_paragraph_mark() {
    for length in [2047, 2048] {
        let text = "x".repeat(length);
        assert_eq!(
            read_through_the_bound("RICHEDIT50W", format!("{text}\r")),
            LocatorField::Known(text),
            "{length} characters of content"
        );
    }
    assert_eq!(
        read_through_the_bound("RICHEDIT50W", format!("{}\r", "x".repeat(2049))),
        LocatorField::Unknown
    );
}

#[test]
fn a_value_past_the_bound_is_unread_never_cut_to_a_prefix() {
    assert_eq!(
        read_through_the_bound("Edit", "x".repeat(2049)),
        LocatorField::Unknown
    );
    assert_eq!(
        read_through_the_bound("RICHEDIT50W", "x".repeat(2050)),
        LocatorField::Unknown
    );
    assert_eq!(
        read_through_the_bound("Edit", "x".repeat(2048)),
        LocatorField::Known("x".repeat(2048))
    );
}

fn range_read(
    value: PropertyOutcome,
    available: PropertyOutcome,
    range: PropertyOutcome,
) -> LocatorField<String> {
    ElementProperties::from_reads(vec![
        (TreeProperty::Value, value),
        (TreeProperty::RangeValueAvailable, available),
        (TreeProperty::RangeValue, range),
    ])
    .locator_evidence(ResolvedVocabulary::unknown())
    .value
}

fn flag(value: bool) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Flag(value))
}

fn real(value: f64) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Real(value))
}

/// A slider that offers RangeValue and no Value pattern answers `Value` as
/// not supported, so its value was empty. It reads as the number, in the
/// text the other platforms report for a slider.
#[test]
fn a_range_control_without_a_value_pattern_reports_its_number_as_the_value() {
    assert_eq!(
        range_read(PropertyOutcome::Absent, flag(true), real(50.0)),
        LocatorField::Known("50".into())
    );
    assert_eq!(
        range_read(PropertyOutcome::Absent, flag(true), real(0.25)),
        LocatorField::Known("0.25".into())
    );
}

#[test]
fn a_range_number_is_not_a_value_without_its_gate_or_when_it_is_not_finite() {
    assert_eq!(
        range_read(PropertyOutcome::Absent, flag(false), real(50.0)),
        LocatorField::Absent
    );
    assert_eq!(
        range_read(
            PropertyOutcome::Absent,
            PropertyOutcome::Unknown,
            real(50.0)
        ),
        LocatorField::Absent
    );
    assert_eq!(
        range_read(PropertyOutcome::Absent, flag(true), real(f64::NAN)),
        LocatorField::Absent
    );
}

#[test]
fn a_text_value_wins_over_a_range_number() {
    assert_eq!(
        range_read(
            PropertyOutcome::Known(PropertyValue::Text("high".into())),
            flag(true),
            real(50.0)
        ),
        LocatorField::Known("high".into())
    );
}
