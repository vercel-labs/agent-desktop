use crate::{AdapterError, ErrorCode};

impl AdapterError {
    /// Whether a poll should read again after this failure: the error says so
    /// itself, or it is a transient code (TIMEOUT, ELEMENT_NOT_FOUND,
    /// APP_UNRESPONSIVE) that it does not declare final.
    pub fn is_retryable_poll_failure(&self) -> bool {
        self.is_explicitly_retryable()
            || (matches!(
                self.code,
                ErrorCode::Timeout | ErrorCode::ElementNotFound | ErrorCode::AppUnresponsive
            ) && self.permits_retry_by_default())
    }
}

#[cfg(test)]
#[path = "poll_retry_tests.rs"]
mod tests;
