use serde_json::json;

use crate::{AdapterError, ErrorCode, RefEntry};

/// The incomplete-and-retryable answer shared by both resolvers: a candidate
/// that could not be read is not a non-match. `RefEntry` evidence is
/// platform-neutral, and an unreadable candidate means the same thing
/// whichever adapter's search produced it, so the payload - message,
/// suggestion, and the `complete: false, retryable: true` stamp - is decided
/// once, here, instead of by two copies that can drift apart.
pub fn identity_unknown_error(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::Timeout,
        "Strict resolution could not determine candidate identity from the live accessibility evidence",
    )
    .with_suggestion(
        "Retry after the target application finishes updating its accessibility tree. \
         A large window can need more time than the budget allows, so retry the \
         same command with a larger --timeout-ms.",
    )
    .with_disposition(crate::DeliverySemantics::not_delivered())
    .with_details(json!({
        "kind": "resolution_identity_unknown",
        "role": entry.identity.role,
        "complete": false,
        "retryable": true,
    }))
}

/// A tokenless ref captured from one of two or more identical elements. A
/// complete read saw the others, so a new snapshot refuses it again.
pub(crate) fn identity_twins(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::AmbiguousTarget,
        "Saved ref names one of two or more identical elements in the window, so it cannot be \
         re-identified safely",
    )
    .with_suggestion(format!(
        "Every snapshot refuses identical twins. Act on a control that is unique in the window. {}",
        crate::headed_mode_suggestion("moves the pointer to choose one with mouse-click --xy, where physical input is available")
    ))
    .with_details(json!({
        "kind": "identity_not_unique",
        "retryable": false,
        "role": entry.identity.role,
    }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

/// A tokenless ref whose capture could not prove it unique: the snapshot ran out
/// of its budget before it read the whole window, or a node left a recorded field
/// unread. More budget can prove it.
pub(crate) fn identity_unproven(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::AmbiguousTarget,
        "Saved ref was not proven unique within the snapshot budget, so it cannot be \
         re-identified safely",
    )
    .with_suggestion(
        "The snapshot reported identity_unproven. Retake it with a larger --timeout-ms, and pass \
         the same larger --timeout-ms to this command, which reads the whole window again.",
    )
    .with_details(json!({
        "kind": "identity_unproven",
        "retryable": false,
        "role": entry.identity.role,
    }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

/// Stamps the final incomplete diagnosis with `deadline_elapsed` so the
/// caller sees why the retries ran out, preserving the incomplete's own
/// details rather than discarding them for a bare `TIMEOUT`. Shared by both
/// resolvers' retry loops, which reach the same terminal shape when their own
/// deadline expires with an incomplete answer still on hand.
pub fn mark_deadline_elapsed(mut error: AdapterError) -> AdapterError {
    let mut details = error.details.take().unwrap_or_else(|| json!({}));
    if let Some(object) = details.as_object_mut() {
        object.insert("deadline_elapsed".into(), json!(true));
    } else {
        details = json!({
            "evidence": details,
            "deadline_elapsed": true,
        });
    }
    error.with_details(details)
}

pub(crate) fn ambiguous_live_match(entry: &RefEntry) -> AdapterError {
    AdapterError::ambiguous_target(
        "Two or more live elements match the saved ref's recorded role and stable identity",
    )
    .with_suggestion(format!(
        "An element identical to the captured one appeared, so neither can be picked by ref and \
         a new snapshot refuses both as twins. Act on a control that is unique in the window. {}",
        crate::headed_mode_suggestion(
            "moves the pointer to choose one by coordinate click, where physical input is available"
        )
    ))
    .with_details(json!({
        "kind": "saved_ref_ambiguous",
        "retryable": false,
        "complete": true,
        "role": entry.identity.role,
    }))
}

pub(crate) fn absent(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref_because("No live element matches the saved ref's recorded identity")
        .with_details(json!({
            "kind": "saved_ref_absent",
            "retryable": false,
            "complete": true,
            "role": entry.identity.role,
        }))
}

#[cfg(test)]
#[path = "resolve_errors_tests.rs"]
mod tests;

/// The answer for an element token that no longer names the element it was
/// taken from. `reason` names the check that failed; `depth` locates a broken
/// ancestor link where the adapter checks one. No search ran, so the ref
/// cannot have landed on a look-alike.
pub fn element_token_stale(entry: &RefEntry, reason: &str, depth: Option<usize>) -> AdapterError {
    let mut details = json!({
        "kind": "retained_object_stale",
        "retryable": false,
        "reason": reason,
        "pid": entry.process.pid,
    });
    if let Some(depth) = depth {
        details["depth"] = depth.into();
    }
    AdapterError::stale_ref_because(
        "The saved element no longer exists where its snapshot found it, or no longer matches",
    )
    .with_details(details)
}

/// The answer for a ref handed to a resolver that re-identifies elements only
/// through the element token captured with them, when the ref carries none.
/// Nothing is searched for it.
pub fn element_token_required(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::StaleRef,
        "This saved ref is re-identified only through the element token captured with it, \
         and it carries none",
    )
    .with_suggestion(
        "Run 'snapshot' to take a ref that carries an element token, then retry with the new ref",
    )
    .with_disposition(crate::DeliverySemantics::not_delivered())
    .with_details(json!({
        "kind": "element_token_required",
        "retryable": false,
        "role": entry.identity.role,
    }))
}
