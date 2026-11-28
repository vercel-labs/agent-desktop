use super::*;
use agent_desktop_core::{MouseButton, MouseEvent, MouseEventKind, Point};

#[test]
fn the_event_count_keeps_the_verification_scope_the_report_carried() {
    let event = MouseEvent {
        kind: MouseEventKind::Move,
        point: Point { x: 10.0, y: 20.0 },
        button: MouseButton::Left,
        modifiers: Vec::new(),
    };
    let report = with_event_count(
        OperationReport::pointer_dispatched(&event, true),
        &Delivery {
            delivered_events: 2,
        },
    );
    let details = report.result.details.expect("details");
    assert_eq!(details["verification_scope"], "pointer_position");
    assert_eq!(details["delivered_events"], 2);
}
