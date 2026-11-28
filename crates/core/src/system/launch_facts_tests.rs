use super::*;

fn facts(
    attached: bool,
    activation_dispatched: bool,
    postcondition_observed: bool,
) -> DeliverySemantics {
    LaunchFacts {
        attached,
        activation_dispatched,
        postcondition_observed,
    }
    .disposition()
}

#[test]
fn attaching_without_an_activation_dispatches_nothing() {
    assert_eq!(
        facts(true, false, false),
        DeliverySemantics::not_delivered()
    );
}

#[test]
fn an_idle_attach_never_claims_a_verified_postcondition() {
    assert_eq!(facts(true, false, true), DeliverySemantics::not_delivered());
}

#[test]
fn attaching_with_an_activation_is_delivered_unverified() {
    assert_eq!(
        facts(true, true, false),
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn attaching_with_an_activation_confirmed_frontmost_is_delivered_verified() {
    assert_eq!(
        facts(true, true, true),
        DeliverySemantics::delivered_verified()
    );
}

#[test]
fn a_spawn_whose_postcondition_was_read_back_is_delivered_verified() {
    assert_eq!(
        facts(false, false, true),
        DeliverySemantics::delivered_verified()
    );
}

#[test]
fn a_spawn_with_no_postcondition_read_back_is_delivered_unverified() {
    assert_eq!(
        facts(false, false, false),
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn a_fresh_launch_with_activation_still_needs_the_postcondition_read() {
    assert_eq!(
        facts(false, true, false),
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(
        facts(false, true, true),
        DeliverySemantics::delivered_verified()
    );
}
