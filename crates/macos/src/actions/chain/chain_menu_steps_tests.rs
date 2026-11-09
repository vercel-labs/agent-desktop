use std::cell::RefCell;

use agent_desktop_core::{AdapterError, DeliveryObservation, DeliveryOutcome, DeliverySemantics};

use super::first_menu_outcome;

const SENT: DeliveryOutcome = DeliveryOutcome::Delivered(DeliveryObservation::Unverified);
const SEEN: DeliveryOutcome = DeliveryOutcome::Delivered(DeliveryObservation::Verified);
const NOTHING: DeliveryOutcome = DeliveryOutcome::NotDelivered;

fn run(outcomes: &[DeliveryOutcome], menu_open: bool) -> (DeliveryOutcome, Vec<usize>) {
    let ran = RefCell::new(Vec::new());
    let mut routes: Vec<_> = outcomes
        .iter()
        .enumerate()
        .map(|(index, outcome)| {
            let ran = &ran;
            move || {
                ran.borrow_mut().push(index);
                Ok::<_, AdapterError>(*outcome)
            }
        })
        .collect();
    let mut as_dyn: Vec<&mut dyn FnMut() -> Result<DeliveryOutcome, AdapterError>> = routes
        .iter_mut()
        .map(|route| route as &mut dyn FnMut() -> Result<DeliveryOutcome, AdapterError>)
        .collect();
    let outcome = first_menu_outcome(&mut as_dyn, &mut || menu_open).unwrap();
    drop(as_dyn);
    drop(routes);
    (outcome, ran.into_inner())
}

#[test]
fn an_unverified_send_that_never_shows_a_menu_lets_every_route_run_once() {
    assert_eq!(
        run(&[SENT, NOTHING, NOTHING, NOTHING, NOTHING], false),
        (SENT, vec![0, 1, 2, 3, 4])
    );
}

#[test]
fn a_menu_seen_after_an_unverified_send_is_verified_before_another_route_runs() {
    assert_eq!(
        run(&[SENT, NOTHING, NOTHING, NOTHING, NOTHING], true),
        (SEEN, vec![0])
    );
}

#[test]
fn routes_that_send_nothing_leave_the_menu_undelivered_even_when_one_is_open() {
    assert_eq!(
        run(&[NOTHING, NOTHING, NOTHING, NOTHING, NOTHING], true),
        (NOTHING, vec![0, 1, 2, 3, 4])
    );
}

#[test]
fn a_route_that_sees_the_menu_ends_the_search() {
    assert_eq!(run(&[NOTHING, SEEN, SENT], false), (SEEN, vec![0, 1]));
}

#[test]
fn a_route_failing_after_an_unverified_send_keeps_the_send_on_record() {
    let mut first = || Ok::<_, AdapterError>(SENT);
    let mut second = || {
        Err::<DeliveryOutcome, _>(
            AdapterError::internal("route failed")
                .with_disposition(DeliverySemantics::not_delivered()),
        )
    };

    let error = first_menu_outcome(&mut [&mut first, &mut second], &mut || false).unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
