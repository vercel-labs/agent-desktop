use super::*;
use crate::{ProcessId, WindowState};

fn window() -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Action center".into(),
        app: "Shell".into(),
        pid: ProcessId::new(9),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    }
}

#[test]
fn a_surface_that_was_already_presented_is_not_delivered() {
    let opening = SurfaceOpening::Present(window());

    assert_eq!(opening.disposition(), DeliverySemantics::not_delivered());
    assert_eq!(opening.window().id, "w-1");
}

#[test]
fn a_surface_the_adapter_raised_and_observed_is_delivered_verified() {
    let opening = SurfaceOpening::Raised(window());

    assert_eq!(
        opening.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(opening.into_window().id, "w-1");
}
