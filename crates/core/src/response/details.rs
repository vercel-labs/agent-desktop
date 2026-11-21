use serde_json::Value;

/// The details an annotation leaves: two objects merge, a key in `added`
/// winning, so a layer that annotates a result or an error never erases what
/// an inner layer said; anything else is replaced by `added`.
pub(crate) fn merged(existing: Option<Value>, added: Value) -> Value {
    match (existing, added) {
        (Some(Value::Object(mut kept)), Value::Object(added)) => {
            kept.extend(added);
            Value::Object(kept)
        }
        (_, added) => added,
    }
}
