use super::*;
use crate::wait::deadline::VirtualClock;
use serde_json::json;

fn incomplete() -> AdapterError {
    AdapterError::timeout("a candidate could not be read")
        .with_details(json!({ "complete": false, "retryable": true }))
}

fn run(
    timeout_ms: u64,
    mut answer: impl FnMut(u32) -> Result<u32, AdapterError>,
) -> (Result<u32, AdapterError>, u32) {
    let _clock = VirtualClock::freeze();
    let deadline = Deadline::after(timeout_ms).unwrap();
    let mut attempts = 0;
    let result = retry_incomplete_until(deadline, || {
        attempts += 1;
        answer(attempts)
    });
    (result, attempts)
}

#[test]
fn an_incomplete_read_is_retried_until_it_settles() {
    let (result, attempts) = run(5_000, |n| if n < 3 { Err(incomplete()) } else { Ok(n) });
    assert_eq!(result.unwrap(), 3);
    assert_eq!(attempts, 3);
}

#[test]
fn settled_and_unstamped_failures_are_returned_after_one_attempt() {
    for error in [
        AdapterError::stale_ref_because("a complete search found nothing"),
        AdapterError::new(ErrorCode::AppUnresponsive, "unstamped transport error"),
        AdapterError::timeout("the provider's own call timed out"),
    ] {
        let code = error.code.clone();
        let (result, attempts) = run(5_000, |_| Err(error.clone()));
        let returned = result.unwrap_err();
        assert_eq!(returned.code, code);
        assert!(
            returned
                .details
                .is_none_or(|d| d.get("deadline_elapsed").is_none())
        );
        assert_eq!(attempts, 1);
    }
}

#[test]
fn a_budget_spent_on_incomplete_reads_returns_the_last_diagnosis_stamped() {
    let (result, attempts) = run(200, |_| Err(incomplete()));
    let error = result.unwrap_err();
    assert!(attempts > 1);
    assert_eq!(error.message, "a candidate could not be read");
    assert_eq!(error.details.unwrap()["deadline_elapsed"], true);
}

#[test]
fn a_timeout_after_an_incomplete_read_returns_the_incomplete_diagnosis() {
    let (result, attempts) = run(5_000, |n| {
        if n == 1 {
            Err(incomplete())
        } else {
            Err(AdapterError::timeout("slice expired"))
        }
    });
    let error = result.unwrap_err();
    assert_eq!(attempts, 2);
    assert_eq!(error.message, "a candidate could not be read");
    assert_eq!(error.details.unwrap()["deadline_elapsed"], true);
}

#[test]
fn an_expired_deadline_never_runs_the_operation() {
    let _clock = VirtualClock::freeze();
    let deadline = Deadline::after(10).unwrap();
    _clock.advance(Duration::from_millis(20));
    let result: Result<(), _> = retry_incomplete_until(deadline, || panic!("must not run"));
    assert_eq!(result.unwrap_err().details.unwrap()["kind"], "deadline");
}

#[test]
fn the_pause_doubles_from_25_ms_and_holds_at_400_ms() {
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(5_000).unwrap();
    let mut attempts_at = Vec::new();
    let _ = retry_incomplete_until(deadline, || {
        attempts_at.push(clock.elapsed());
        if attempts_at.len() < 8 {
            Err(incomplete())
        } else {
            Ok(())
        }
    });
    let gaps: Vec<u128> = attempts_at
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).as_millis())
        .collect();
    assert_eq!(gaps, [25, 50, 100, 200, 400, 400, 400]);
}
