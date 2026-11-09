use serde_json::Value;

const VARIES: &str = "<varies>";

fn redact(value: &mut Value, varying: &[&str], seen: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map.iter_mut() {
                if varying.contains(&key.as_str()) {
                    seen.push(key.clone());
                    *inner = match inner {
                        Value::Number(_) => Value::from(0),
                        Value::Bool(_) => Value::Bool(false),
                        Value::Array(_) => Value::Array(Vec::new()),
                        Value::Object(_) => Value::Object(Default::default()),
                        Value::Null => Value::Null,
                        Value::String(_) => Value::String(VARIES.to_string()),
                    };
                } else {
                    redact(inner, varying, seen);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| redact(item, varying, seen)),
        _ => {}
    }
}

/// Compares `actual` with the golden JSON text by value, so a `null` and an absent key differ.
/// Every key in `varying` must exist in `actual` and is replaced before the comparison by the
/// placeholder of its JSON type: `"<varies>"` for a string, `0` for a number, `false`, `[]`, `{}`
/// or `null`. The golden holds that placeholder at the same place, so it pins the type.
pub(super) fn assert_golden(mut actual: Value, golden: &str, varying: &[&str]) {
    let mut seen = Vec::new();
    redact(&mut actual, varying, &mut seen);
    for key in varying {
        assert!(
            seen.iter().any(|found| found == key),
            "the varying key {key:?} is missing from the output:\n{}",
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        );
    }
    let expected: Value = serde_json::from_str(golden).expect("the golden file is valid JSON");
    assert_eq!(
        actual,
        expected,
        "output differs from the golden file. Actual:\n{}",
        serde_json::to_string_pretty(&actual).unwrap_or_default()
    );
}
