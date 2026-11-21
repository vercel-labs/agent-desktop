use super::*;
use serde_json::json;

fn unstamped(code: ErrorCode) -> AdapterError {
    AdapterError::new(code, "failure")
}

fn stamped(code: ErrorCode, retryable: bool) -> AdapterError {
    unstamped(code).with_details(json!({ "retryable": retryable }))
}

#[test]
fn a_poll_reads_again_only_for_a_stamped_or_a_transient_failure_that_is_not_final() {
    let cases = [
        (unstamped(ErrorCode::Timeout), true),
        (unstamped(ErrorCode::ElementNotFound), true),
        (unstamped(ErrorCode::AppUnresponsive), true),
        (stamped(ErrorCode::Timeout, false), false),
        (stamped(ErrorCode::ElementNotFound, false), false),
        (stamped(ErrorCode::AppUnresponsive, false), false),
        (stamped(ErrorCode::StaleRef, true), true),
        (stamped(ErrorCode::StaleRef, false), false),
        (unstamped(ErrorCode::StaleRef), false),
        (unstamped(ErrorCode::PermDenied), false),
        (unstamped(ErrorCode::Internal), false),
    ];

    for (error, expected) in cases {
        assert_eq!(
            error.is_retryable_poll_failure(),
            expected,
            "{} with {:?}",
            error.code.as_str(),
            error.details
        );
    }
}
