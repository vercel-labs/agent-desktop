use super::LEGACY_VALUE_LABEL;
use super::imp::gated_value_compare;
use crate::tree::properties::property_outcome::{PropertyOutcome, PropertyValue};
use agent_desktop_core::DeliveryOutcome;
use agent_desktop_core::{Action, DeliveryDisposition};
use std::cell::Cell;

fn known_flag(value: bool) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Flag(value))
}

fn code_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("///") || trimmed.starts_with("//!"))
        })
        .map(|(index, line)| (index + 1, line))
}

#[test]
fn secure_is_password_skips_get_value_and_reports_unobserved() {
    let reads = Cell::new(0u8);
    let verified = gated_value_compare(known_flag(true), "secret-marker-zz", || {
        reads.set(reads.get() + 1);
        Ok("secret-marker-zz".into())
    })
    .expect("gate");
    assert_eq!(verified, None);
    assert_eq!(reads.get(), 0);

    let steps = vec![(DeliveryOutcome::observed(None)).step(LEGACY_VALUE_LABEL)];
    assert_eq!(steps[0].verified(), None);
    let result = agent_desktop_core::ActionResult::from_execution(
        &Action::SetValue("secret-marker-zz".into()),
        steps,
    );
    assert_eq!(
        result.disposition().delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
}

#[test]
fn unknown_is_password_withholds_identically() {
    let reads = Cell::new(0u8);
    let verified = gated_value_compare(PropertyOutcome::Unknown, "marker", || {
        reads.set(reads.get() + 1);
        Ok("marker".into())
    })
    .expect("unknown withholds");
    assert_eq!(verified, None);
    assert_eq!(reads.get(), 0);
}

#[test]
fn known_false_is_password_reads_and_compares() {
    let reads = Cell::new(0u8);
    let verified = gated_value_compare(known_flag(false), "abc", || {
        reads.set(reads.get() + 1);
        Ok("abc".into())
    })
    .expect("readable");
    assert_eq!(verified, Some(true));
    assert_eq!(reads.get(), 1);

    let mismatch =
        gated_value_compare(known_flag(false), "abc", || Ok("xyz".into())).expect("mismatch");
    assert_eq!(mismatch, Some(false));
}

#[test]
fn inverted_secure_gate_would_call_get_value() {
    let reads = Cell::new(0u8);
    let _ = gated_value_compare(known_flag(false), "x", || {
        reads.set(reads.get() + 1);
        Ok("x".into())
    });
    assert_eq!(
        reads.get(),
        1,
        "flipping the gate to Known(false) must observe a get_value"
    );
}

#[test]
fn pattern_get_value_lives_only_inside_value_write_gate() {
    let actions_sources = crate::actions::mutation::action_scan_sources();
    let get_value = concat!(".", "get_value(");
    for (name, source) in actions_sources {
        for (number, line) in code_lines(source) {
            if name.ends_with("value_write.rs") {
                continue;
            }
            assert!(
                !line.contains(get_value),
                "{name}:{number} must not call get_value outside the gate: {line}"
            );
        }
    }
    let value_write = include_str!("value_write.rs");
    assert!(
        value_write.contains(get_value),
        "value_write.rs must own the get_value call sites"
    );
    assert!(
        value_write.contains("gated_pattern_value_equals")
            && value_write.contains("gated_pattern_range_equals"),
        "get_value must live inside the gated helpers"
    );
}

#[test]
fn a_planted_get_value_outside_the_gate_is_caught() {
    let get_value = concat!(".", "get_value(");
    let fixture = "let _ = pattern.get_value();";
    assert!(fixture.contains(get_value));
    let offences = code_lines(fixture)
        .filter(|(_, line)| line.contains(get_value))
        .count();
    assert_eq!(
        offences, 1,
        "MUST-CATCH: planted get_value outside the gate must fail the scan"
    );
}
