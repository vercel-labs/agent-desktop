use super::*;
use crate::wait::deadline::VirtualClock;

fn cut_short(cause: IncompleteCause) -> Result<DrillAttempt<u32>, AppError> {
    Ok(DrillAttempt::Incomplete(IncompleteRead {
        nodes_observed: 40,
        cause,
    }))
}

fn details(error: AppError) -> (String, serde_json::Value) {
    match error {
        AppError::Adapter(adapter) => (adapter.message, adapter.details.expect("details")),
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

#[test]
fn a_read_the_application_changed_is_read_again_until_it_is_whole() {
    let _clock = VirtualClock::freeze();
    let deadline = Deadline::after(5_000).unwrap();
    let mut reads = 0;
    let value = drill_until_complete(deadline, || {
        reads += 1;
        if reads < 3 {
            cut_short(IncompleteCause::LiveChurn)
        } else {
            Ok(DrillAttempt::Complete(7))
        }
    })
    .expect("the third read is whole");
    assert_eq!((value, reads), (7, 3));
}

#[test]
fn a_subtree_that_never_reads_whole_fails_only_after_the_whole_timeout() {
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(50).unwrap();
    let mut reads = 0;
    let error = drill_until_complete(deadline, || {
        reads += 1;
        cut_short(IncompleteCause::LiveChurn)
    })
    .expect_err("nothing ever reads whole");
    assert!(clock.elapsed() >= Duration::from_millis(50));
    assert!(reads > 1);
    let (message, details) = details(error);
    assert!(message.contains("kept changing"), "{message}");
    assert_eq!(details["walks_finished"], reads);
}

#[test]
fn a_deterministic_bound_cut_returns_at_once_without_blaming_churn() {
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(5_000).unwrap();
    let mut reads = 0;
    let error = drill_until_complete(deadline, || {
        reads += 1;
        cut_short(IncompleteCause::Bounded)
    })
    .expect_err("a bounded cut cannot become whole on retry");
    assert_eq!(reads, 1);
    assert_eq!(clock.elapsed(), Duration::ZERO);
    let (message, details) = details(error);
    assert!(message.contains("observation bound"), "{message}");
    assert_eq!(details["attempts"], 1);
    assert_eq!(details["walks_finished"], 0);
}

#[test]
fn a_read_that_only_ever_ran_out_of_time_blames_the_timeout() {
    let _clock = VirtualClock::freeze();
    let deadline = Deadline::after(30).unwrap();
    let error = drill_until_complete(deadline, || cut_short(IncompleteCause::Deadline))
        .expect_err("never whole");
    let (message, details) = details(error);
    assert!(message.contains("ran out of time"), "{message}");
    assert_eq!(details["walks_finished"], 0);
}

#[test]
fn an_error_from_a_read_is_not_retried() {
    let deadline = Deadline::after(5_000).unwrap();
    let mut reads = 0;
    let error = drill_until_complete::<u32>(deadline, || {
        reads += 1;
        Err(AppError::stale_ref("@s:e1"))
    })
    .expect_err("an error ends the drill");
    assert_eq!(reads, 1);
    assert!(!error.to_string().is_empty());
}
