use agent_desktop_core::{WindowOp, WindowReadback};

use crate::system::window::window_postcondition::TOLERANCE;

pub(crate) fn window_readback(op: &WindowOp) -> Option<WindowReadback> {
    (!matches!(op, WindowOp::Maximize))
        .then(|| WindowReadback::for_window_op(op, TOLERANCE, "points"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{
        DeliverySemantics, OperationReport, OperationTarget, ProcessIdentity,
    };

    fn target() -> OperationTarget {
        OperationTarget::Window {
            id: "w-1".into(),
            title: "Doc".into(),
            process: ProcessIdentity::new(7u32, "boot"),
        }
    }

    fn report(op: &WindowOp) -> OperationReport {
        OperationReport::window_operation(target(), op, window_readback(op))
    }

    #[test]
    fn maximize_stays_unverified_because_position_and_size_are_read_separately() {
        assert_eq!(
            report(&WindowOp::Maximize).disposition(),
            DeliverySemantics::delivered_unverified()
        );
    }

    #[test]
    fn geometry_scopes_name_exactly_what_was_read_with_the_real_tolerance() {
        let resize = report(&WindowOp::Resize {
            width: 3.0,
            height: 4.0,
        });
        let moved = report(&WindowOp::Move { x: 1.0, y: 2.0 });

        let resize = resize.result.details.unwrap();
        let moved = moved.result.details.unwrap();
        assert_eq!(resize["verification_scope"], "window_size");
        assert_eq!(moved["verification_scope"], "window_position");
        for details in [resize, moved] {
            assert_eq!(details["tolerance"], TOLERANCE);
            assert_eq!(details["units"], "points");
        }
    }
}
