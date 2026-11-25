use super::*;
use crate::DeliverySemantics;

fn disposition(arrival: LaunchArrival) -> DeliverySemantics {
    arrival.facts().disposition()
}

#[test]
fn a_started_launch_is_classified_by_who_owns_the_windows() {
    assert_eq!(LaunchArrival::started(true, true), LaunchArrival::Spawned);
    assert_eq!(LaunchArrival::started(false, true), LaunchArrival::Appeared);
    assert_eq!(
        LaunchArrival::started(false, false),
        LaunchArrival::Forwarded
    );
    assert!(LaunchArrival::Spawned.created());
    assert!(!LaunchArrival::Appeared.created());
}

#[test]
fn only_a_new_instance_is_verified_and_only_an_idle_attach_is_not_delivered() {
    assert_eq!(
        disposition(LaunchArrival::Attached),
        DeliverySemantics::not_delivered()
    );
    assert_eq!(
        disposition(LaunchArrival::Spawned),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(
        disposition(LaunchArrival::Appeared),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(
        disposition(LaunchArrival::Forwarded),
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(
        disposition(LaunchArrival::Unobserved),
        DeliverySemantics::delivered_unverified()
    );
}
