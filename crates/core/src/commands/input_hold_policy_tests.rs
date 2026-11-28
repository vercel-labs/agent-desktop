use super::*;
use crate::DeliverySemantics;

#[test]
fn a_hold_refusal_is_not_delivered_and_safe_to_retry() {
    let AppError::Adapter(error) = reject("key-down", "press") else {
        panic!("the hold refusal is an adapter error");
    };

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.disposition.retry(), crate::RetryDisposition::Safe);
}
