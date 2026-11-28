use agent_desktop_core::{AdapterError, Deadline};
use serde_json::json;
use std::time::Duration;

/// What is left of `deadline`, or a retryable timeout once it has run out: an
/// exhausted slice of a live read is an incomplete read the caller may retry
/// while its outer budget lasts.
pub(crate) fn remaining(deadline: Deadline) -> Result<Duration, AdapterError> {
    let remaining = deadline.remaining();
    if remaining.is_zero() {
        return Err(
            AdapterError::timeout("Live locator deadline exhausted").with_details(json!({
                "kind": "locator_deadline_exhausted",
                "retryable": true,
            })),
        );
    }
    Ok(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exhausted_deadline_is_a_retryable_incomplete_read() {
        let deadline = Deadline::after(1).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        let error = remaining(deadline).expect_err("deadline must be exhausted");
        assert!(error.is_transient_incomplete_read());
        assert!(remaining(Deadline::after(1_000).unwrap()).is_ok());
    }
}
