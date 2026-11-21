//! LegacyIAccessible, ValuePattern and RangeValuePattern writes for
//! `SetValue` and `Clear`.
//!
//! Verification re-reads route through [`gated_pattern_value_equals`] /
//! [`gated_pattern_range_equals`] so an `IsPassword` element never calls
//! `get_value` (A19-3). Error echoes carry `value_chars` counts, never text.

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
};

use crate::actions::chain::{offered, rung};

use crate::tree::element::UIAElement;
use agent_desktop_core::DeliveryOutcome;

pub(crate) const VALUE_LABEL: &str = "ValuePattern.SetValue";
pub(crate) const RANGE_LABEL: &str = "RangeValuePattern.SetValue";
pub(crate) const LEGACY_VALUE_LABEL: &str = "LegacyIAccessible.SetValue";

const VALUE_WRITE_SUGGESTION: &str =
    "Refresh the snapshot and retry, or target a writable Value or RangeValue control.";

#[cfg(target_os = "windows")]
mod imp {
    use super::without_terminal_paragraph;
    use super::{
        ActionStep, AdapterError, Deadline, DeliveryOutcome, DeliverySemantics, ErrorCode,
        InteractionPolicy, LEGACY_VALUE_LABEL, RANGE_LABEL, UIAElement, VALUE_LABEL,
        VALUE_WRITE_SUGGESTION, offered, rung,
    };
    use crate::actions::chain::{after_delivery, legacy_set_value, poll_until};
    use agent_desktop_core::DeliveryObservation;

    /// How long a Legacy `SetValue` gets to show the value before the write
    /// counts as acknowledged with no effect, so a headed pattern rung may try.
    const LEGACY_SETTLE: std::time::Duration = std::time::Duration::from_millis(600);
    use crate::actions::mutation::{classify_success, classify_write};
    use crate::tree::properties::element_properties::ElementProperties;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::property_outcome::PropertyOutcome;
    use crate::tree::properties::read_one;
    use uiautomation::patterns::{UIRangeValuePattern, UIValuePattern};

    pub(crate) fn clear_steps(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        set_value_steps(element, "", policy, deadline)
    }

    /// Writes `value` through LegacyIAccessible `SetValue`, which lands with
    /// no foreground change on Win32 and RichEdit edits, WinForms and WPF
    /// text boxes, a WinForms TrackBar and a WPF Slider (C4, P2), then
    /// through ValuePattern or RangeValuePattern, which activate the window
    /// and are headed rungs. Every delivery is verified by re-reading the
    /// value through the IsPassword gate.
    pub(crate) fn set_value_steps(
        element: &UIAElement,
        value: &str,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let parsed = parse_finite_f64(value);
        let text_ok = value_writable(element);
        let range_ok = range_writable(element) && parsed.is_some();
        let observe = |delivered: bool| -> Result<DeliveryOutcome, AdapterError> {
            if !delivered {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            let verified = match parsed.filter(|_| !text_ok) {
                Some(target) => gated_pattern_range_equals(element, target),
                None => gated_pattern_value_equals(element, value),
            }
            .map_err(after_delivery)?;
            Ok(DeliveryOutcome::observed(verified))
        };
        let mut legacy = || {
            if !legacy_set_value(element, value)? {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            let mut last = Ok(None);
            poll_until(deadline, LEGACY_SETTLE, || {
                last = match parsed.filter(|_| !text_ok) {
                    Some(target) => gated_pattern_range_equals(element, target),
                    None => gated_pattern_value_equals(element, value),
                };
                !matches!(last, Ok(Some(false)))
            })
            .map_err(after_delivery)?;
            Ok(match last.map_err(after_delivery)? {
                Some(false) => DeliveryOutcome::Delivered(DeliveryObservation::NoEffect),
                read => DeliveryOutcome::observed(read),
            })
        };
        let mut text = || observe(invoke_value_set(element, value)?);
        let mut range = || match parsed {
            Some(target) => observe(invoke_range_set(element, target)?),
            None => Ok(DeliveryOutcome::NotDelivered),
        };
        agent_desktop_core::execute_chain(
            &mut offered([
                rung(LEGACY_VALUE_LABEL, text_ok || range_ok, false, &mut legacy),
                rung(VALUE_LABEL, text_ok, true, &mut text),
                rung(RANGE_LABEL, range_ok, true, &mut range),
            ]),
            policy,
            deadline,
            VALUE_WRITE_SUGGESTION,
        )
        .map_err(|error| attach_value_chars(error, value))
    }

    fn value_writable(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::ValueAvailable).flag() == Some(true)
            && read_one(element, TreeProperty::ValueIsReadOnly).flag() == Some(false)
    }

    fn range_writable(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::RangeValueAvailable).flag() == Some(true)
            && read_one(element, TreeProperty::RangeValueIsReadOnly).flag() == Some(false)
    }

    pub(crate) fn parse_finite_f64(value: &str) -> Option<f64> {
        value
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
    }

    fn invoke_value_set(element: &UIAElement, value: &str) -> Result<bool, AdapterError> {
        match element.0.get_pattern::<UIValuePattern>() {
            Ok(pattern) => match pattern.set_value(value) {
                Ok(()) => classify_success(),
                Err(error) => classify_write("SetValue", VALUE_LABEL, &error),
            },
            Err(error) => classify_write("get_pattern", VALUE_LABEL, &error),
        }
    }

    fn invoke_range_set(element: &UIAElement, target: f64) -> Result<bool, AdapterError> {
        match element.0.get_pattern::<UIRangeValuePattern>() {
            Ok(pattern) => match pattern.set_value(target) {
                Ok(()) => classify_success(),
                Err(error) => classify_write("SetValue", RANGE_LABEL, &error),
            },
            Err(error) => classify_write("get_pattern", RANGE_LABEL, &error),
        }
    }

    /// IsPassword-gated ValuePattern readback. The only `UIValuePattern::get_value`
    /// call site under `actions/`.
    pub(crate) fn gated_pattern_value_equals(
        element: &UIAElement,
        expected: &str,
    ) -> Result<Option<bool>, AdapterError> {
        let rich_edit = ElementProperties::from_reads(vec![(
            TreeProperty::ClassName,
            read_one(element, TreeProperty::ClassName),
        )])
        .is_rich_edit();
        gated_value_compare(
            read_one(element, TreeProperty::IsPassword),
            expected,
            || {
                let pattern = element
                    .0
                    .get_pattern::<UIValuePattern>()
                    .map_err(|error| read_failed("ValuePattern.get_value", &error))?;
                pattern
                    .get_value()
                    .map(|text| {
                        if rich_edit {
                            without_terminal_paragraph(text, expected)
                        } else {
                            text
                        }
                    })
                    .map_err(|error| read_failed("ValuePattern.get_value", &error))
            },
        )
    }

    /// IsPassword-gated RangeValuePattern readback. The only
    /// `UIRangeValuePattern::get_value` call site under `actions/`.
    pub(crate) fn gated_pattern_range_equals(
        element: &UIAElement,
        expected: f64,
    ) -> Result<Option<bool>, AdapterError> {
        gated_range_compare(
            read_one(element, TreeProperty::IsPassword),
            expected,
            || {
                let pattern = element
                    .0
                    .get_pattern::<UIRangeValuePattern>()
                    .map_err(|error| read_failed("RangeValuePattern.get_value", &error))?;
                pattern
                    .get_value()
                    .map_err(|error| read_failed("RangeValuePattern.get_value", &error))
            },
        )
    }

    /// Shared secure-field gate: withholds the comparison on a secure field
    /// rather than reading it, otherwise reads once and compares. Generic
    /// over the compared type so [`gated_value_compare`]'s `String` read
    /// against a `&str` expectation and [`gated_range_compare`]'s `f64`
    /// read against an `f64` expectation share one body.
    fn gated_compare<T, U>(
        is_password: PropertyOutcome,
        expected: U,
        mut read: impl FnMut() -> Result<T, AdapterError>,
    ) -> Result<Option<bool>, AdapterError>
    where
        T: PartialEq<U>,
    {
        if withholds_value_read(&is_password) {
            return Ok(None);
        }
        let observed = read()?;
        Ok(Some(observed == expected))
    }

    /// Shared secure-field gate for string value verification (reusable by Select).
    pub(crate) fn gated_value_compare(
        is_password: PropertyOutcome,
        expected: &str,
        read: impl FnMut() -> Result<String, AdapterError>,
    ) -> Result<Option<bool>, AdapterError> {
        gated_compare(is_password, expected, read)
    }

    /// Shared secure-field gate for numeric range verification.
    pub(crate) fn gated_range_compare(
        is_password: PropertyOutcome,
        expected: f64,
        read: impl FnMut() -> Result<f64, AdapterError>,
    ) -> Result<Option<bool>, AdapterError> {
        gated_compare(is_password, expected, read)
    }

    fn withholds_value_read(is_password: &PropertyOutcome) -> bool {
        ElementProperties::from_reads(vec![(TreeProperty::IsPassword, is_password.clone())])
            .is_secure()
    }

    fn read_failed(api: &str, error: &uiautomation::Error) -> AdapterError {
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!("{api} could not re-read the control value after delivery"),
        )
        .with_platform_detail(format!("{api}: {error}"))
        .with_disposition(DeliverySemantics::delivered_unverified())
    }

    fn attach_value_chars(error: AdapterError, value: &str) -> AdapterError {
        error.with_details(serde_json::json!({
            "value_chars": value.chars().count(),
        }))
    }
}

/// A RichEdit control always ends its text with a paragraph mark, so writing
/// `abc` reads back `abc\r` (P2); the mark is not part of the written value.
/// Applied only to RichEdit classes, the same rule the tree value read uses.
pub(crate) fn without_terminal_paragraph(text: String, expected: &str) -> String {
    match text.strip_suffix('\r') {
        Some(stripped) if !expected.ends_with('\r') => stripped.to_string(),
        _ => text,
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionStep, AdapterError, Deadline, InteractionPolicy, UIAElement};
    use crate::tree::properties::property_outcome::PropertyOutcome;

    pub(crate) fn set_value_steps(
        _element: &UIAElement,
        _value: &str,
        _policy: InteractionPolicy,
        _deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("SetValue"))
    }

    pub(crate) fn clear_steps(
        _element: &UIAElement,
        _policy: InteractionPolicy,
        _deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Clear"))
    }

    pub(crate) fn gated_pattern_value_equals(
        _element: &UIAElement,
        _expected: &str,
    ) -> Result<Option<bool>, AdapterError> {
        Err(AdapterError::not_supported("gated_pattern_value_equals"))
    }

    pub(crate) fn gated_value_compare(
        _is_password: PropertyOutcome,
        _expected: &str,
        _read: impl FnMut() -> Result<String, AdapterError>,
    ) -> Result<Option<bool>, AdapterError> {
        Err(AdapterError::not_supported("gated_value_compare"))
    }
}

pub(crate) use imp::{clear_steps, set_value_steps};

#[allow(unused_imports)]
pub(crate) use imp::{gated_pattern_value_equals, gated_value_compare};

#[cfg(all(test, target_os = "windows"))]
pub(crate) use imp::parse_finite_f64;

#[cfg(all(test, target_os = "windows"))]
#[path = "value_write_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "value_write_gate_tests.rs"]
mod gate_tests;
