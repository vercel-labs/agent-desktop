use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

#[test]
fn retry_transient_read_recovers_from_retryable_timeout_and_stops_on_terminal() {
    let busy = || {
        AdapterError::timeout("busy")
            .with_details(serde_json::json!({ "complete": false, "retryable": true }))
    };
    let mut attempts = 0_u32;
    let recovered = super::retry_transient_read(Deadline::after(2_000).unwrap(), || {
        attempts += 1;
        if attempts < 3 {
            Err(busy())
        } else {
            Ok(42_u32)
        }
    });
    assert_eq!((recovered.unwrap(), attempts), (42, 3));

    let mut terminal_attempts = 0_u32;
    let terminal = super::retry_transient_read(Deadline::after(2_000).unwrap(), || {
        terminal_attempts += 1;
        Err::<u32, _>(AdapterError::new(ErrorCode::PermDenied, "denied"))
    });
    assert_eq!(
        (terminal.unwrap_err().code, terminal_attempts),
        (ErrorCode::PermDenied, 1)
    );

    let mut short_attempts = 0_u32;
    let short = super::retry_transient_read(Deadline::after(10).unwrap(), || {
        short_attempts += 1;
        Err::<u32, _>(busy())
    });
    assert_eq!(
        (short.unwrap_err().code, short_attempts),
        (ErrorCode::Timeout, 1)
    );
}
