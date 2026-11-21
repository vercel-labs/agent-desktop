use super::golden::assert_golden;
use serde_json::json;

#[test]
fn a_matching_output_passes() {
    assert_golden(
        json!({"a": 1, "b": [true, null]}),
        include_str!("../../../../../tests/fixtures/contract/golden_selftest.json"),
        &[],
    );
}

#[test]
#[should_panic(expected = "output differs from the golden file")]
fn a_null_does_not_equal_an_absent_key() {
    assert_golden(
        json!({"a": 1, "b": [true, null], "c": null}),
        include_str!("../../../../../tests/fixtures/contract/golden_selftest.json"),
        &[],
    );
}

#[test]
fn a_varying_key_is_replaced_wherever_it_occurs() {
    assert_golden(
        json!({"a": 1, "b": [true, null], "id": "s123", "nested": {"id": "s456"}}),
        include_str!("../../../../../tests/fixtures/contract/golden_selftest_varying.json"),
        &["id"],
    );
}

#[test]
#[should_panic(expected = "is missing from the output")]
fn a_varying_key_that_is_absent_fails() {
    assert_golden(
        json!({"a": 1, "b": [true, null]}),
        include_str!("../../../../../tests/fixtures/contract/golden_selftest.json"),
        &["id"],
    );
}

#[test]
#[should_panic(expected = "output differs from the golden file")]
fn a_varying_key_of_another_json_type_fails() {
    assert_golden(
        json!({"a": 1, "b": [true, null], "id": 123, "nested": {"id": "s456"}}),
        include_str!("../../../../../tests/fixtures/contract/golden_selftest_varying.json"),
        &["id"],
    );
}

#[test]
fn a_varying_integer_and_a_varying_fraction_keep_their_shapes() {
    assert_golden(
        json!({"a": 1, "elapsed": 250, "ratio": 0.75}),
        include_str!("../../../../../tests/fixtures/contract/golden_selftest_fraction.json"),
        &["elapsed", "ratio"],
    );
}

#[test]
#[should_panic(expected = "output differs from the golden file")]
fn a_varying_fraction_does_not_match_an_integer_golden() {
    assert_golden(
        json!({"a": 1, "elapsed": 250.5, "ratio": 0.75}),
        include_str!("../../../../../tests/fixtures/contract/golden_selftest_fraction.json"),
        &["elapsed", "ratio"],
    );
}

#[test]
#[should_panic(expected = "output differs from the golden file")]
fn a_varying_integer_does_not_match_a_fractional_golden() {
    assert_golden(
        json!({"a": 1, "elapsed": 250, "ratio": 3}),
        include_str!("../../../../../tests/fixtures/contract/golden_selftest_fraction.json"),
        &["elapsed", "ratio"],
    );
}
