use serde_json::json;

use crate::{
    AdapterError, Deadline, ErrorCode, NativeHandle,
    refs::resolve_attempt_outcome::ResolveAttemptOutcome,
};

/// Runs one resolve attempt under the command's own deadline.
///
/// A resolve of a tokenless ref reads the whole window, which on a slow
/// accessibility tree costs more than any fixed slice. A slice shorter than the
/// caller's `--timeout-ms` would cut every attempt off mid-read, so the verdict
/// would never settle however much budget the caller gave. The attempt owns the
/// whole remaining budget; the poll loop around it spends the rest on retries of
/// failures that return quickly.
pub(crate) fn attempt_within_deadline(
    deadline: Deadline,
    attempt: impl FnOnce(Deadline) -> Result<NativeHandle, AdapterError>,
) -> ResolveAttemptOutcome {
    if deadline.is_expired() {
        return ResolveAttemptOutcome::DeadlinePassed;
    }
    match attempt(deadline) {
        Ok(_) if deadline.is_expired() => ResolveAttemptOutcome::DeadlinePassed,
        Ok(handle) => ResolveAttemptOutcome::Resolved(handle),
        Err(error) => classify_error(error, deadline),
    }
}

fn classify_error(error: AdapterError, deadline: Deadline) -> ResolveAttemptOutcome {
    if error.code != ErrorCode::Timeout {
        return ResolveAttemptOutcome::Failed(error);
    }
    if !error.permits_retry_by_default()
        || error.disposition.retry() == crate::RetryDisposition::Unsafe
    {
        return ResolveAttemptOutcome::Failed(error);
    }
    if deadline.is_expired() {
        return ResolveAttemptOutcome::DeadlinePassed;
    }
    ResolveAttemptOutcome::Failed(mark_retryable(error))
}

fn mark_retryable(mut error: AdapterError) -> AdapterError {
    let mut details = error.details.take().unwrap_or_else(|| json!({}));
    if let Some(object) = details.as_object_mut() {
        object.insert("retryable".into(), true.into());
    } else {
        details = json!({
            "error_details": details,
            "retryable": true,
        });
    }
    error.with_details(details)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn an_attempt_gets_the_whole_remaining_command_budget() {
        let _clock = crate::wait::deadline::VirtualClock::freeze();
        let deadline = Deadline::after(30_000).unwrap();
        let mut granted = Duration::ZERO;

        let _ = attempt_within_deadline(deadline, |attempt| {
            granted = attempt.remaining();
            Err(AdapterError::timeout("probe"))
        });

        assert_eq!(granted, Duration::from_millis(30_000));
    }

    #[test]
    fn per_attempt_timeout_is_retryable_while_outer_deadline_remains() {
        let error = AdapterError::timeout("strict resolution slice expired")
            .with_details(json!({ "kind": "deadline" }));

        let ResolveAttemptOutcome::Failed(error) =
            attempt_within_deadline(Deadline::after(1_000).unwrap(), |_| Err(error))
        else {
            panic!("per-attempt timeout must remain inside the outer poll loop");
        };

        assert!(error.is_explicitly_retryable());
        assert_eq!(error.details.unwrap()["kind"], "deadline");
    }

    #[test]
    fn explicit_non_retryable_timeout_remains_terminal() {
        let error = AdapterError::timeout("deterministic limit")
            .with_details(json!({ "retryable": false }));

        let ResolveAttemptOutcome::Failed(error) =
            attempt_within_deadline(Deadline::after(1_000).unwrap(), |_| Err(error))
        else {
            panic!("explicit non-retryable timeout must remain terminal");
        };

        assert!(!error.permits_retry_by_default());
    }

    #[test]
    fn uncertain_timeout_remains_terminal_with_stronger_delivery_evidence() {
        let error = AdapterError::timeout("delivery is uncertain")
            .with_disposition(crate::DeliverySemantics::uncertain());

        let ResolveAttemptOutcome::Failed(error) =
            attempt_within_deadline(Deadline::after(1_000).unwrap(), |_| Err(error))
        else {
            panic!("delivery-uncertain timeout must remain terminal");
        };

        assert_eq!(error.disposition, crate::DeliverySemantics::uncertain());
        assert!(!error.is_explicitly_retryable());
    }

    #[test]
    fn outer_deadline_owns_the_terminal_timeout_shape() {
        let clock = crate::wait::deadline::VirtualClock::freeze();
        let deadline = Deadline::after(50).unwrap();

        assert!(matches!(
            attempt_within_deadline(deadline, |_| {
                clock.advance(Duration::from_millis(60));
                Err(AdapterError::timeout("attempt expired"))
            }),
            ResolveAttemptOutcome::DeadlinePassed
        ));
    }
}
