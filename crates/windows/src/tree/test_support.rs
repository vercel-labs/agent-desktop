//! Small, generic test builders shared across otherwise-unrelated test
//! modules - too broadly useful to belong to any one of them, and too small
//! to be worth re-typing in each.

use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::properties::{ElementProperties, PropertyOutcome, PropertyValue};

/// Builds an `ElementProperties` from an explicit read set, the way every
/// producer test stages one node's evidence.
pub(crate) fn props(reads: &[(TreeProperty, PropertyOutcome)]) -> ElementProperties {
    ElementProperties::from_reads(reads.to_vec())
}

pub(crate) fn flag(property: TreeProperty, value: bool) -> (TreeProperty, PropertyOutcome) {
    (property, PropertyOutcome::Known(PropertyValue::Flag(value)))
}

pub(crate) fn number(property: TreeProperty, value: i32) -> (TreeProperty, PropertyOutcome) {
    (
        property,
        PropertyOutcome::Known(PropertyValue::Number(value)),
    )
}

pub(crate) fn text(value: &str) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Text(value.into()))
}

/// Asserts that no non-doc line of any `(name, source)` pair satisfies
/// `is_banned`, skipping `///`/`//!` prose so a comment describing the ban is
/// never mistaken for a violation of it.
pub(crate) fn assert_source_forbids(sources: &[(&str, &str)], is_banned: impl Fn(&str) -> bool) {
    for (name, source) in sources {
        for (number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("///") || trimmed.starts_with("//!") {
                continue;
            }
            assert!(
                !is_banned(line),
                "{name}:{} carries a banned token: {line}",
                number + 1
            );
        }
    }
}
