use super::*;

fn launch_with<'a>(
    bus: &'a SharedBus,
    options: &'a LaunchOptions,
    deadline: Deadline,
) -> Launch<'a> {
    Launch {
        bus,
        id: "fixture",
        options,
        lease: Some(InteractionLease::guarded(deadline, ()).unwrap()),
        deadline,
    }
}

#[test]
fn a_background_launch_hands_its_lease_to_the_focus_watch() {
    let bus = SharedBus::default();
    let options = LaunchOptions::default();
    let mut launch = launch_with(&bus, &options, Deadline::after(1_000).unwrap());
    assert!(launch.watch_lease().is_some());
    assert!(launch.lease.is_none());
}

#[test]
fn an_activating_launch_keeps_the_lease_for_its_focus_request() {
    let bus = SharedBus::default();
    let options = LaunchOptions {
        activate: true,
        ..LaunchOptions::default()
    };
    let mut launch = launch_with(&bus, &options, Deadline::after(1_000).unwrap());
    assert!(launch.watch_lease().is_none());
    assert!(launch.lease.is_some());
}
