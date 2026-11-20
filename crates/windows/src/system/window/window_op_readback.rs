use agent_desktop_core::{WindowOp, WindowReadback};

use super::window_op::PLACEMENT_TOLERANCE_PX;

pub(crate) fn window_readback(op: &WindowOp) -> Option<WindowReadback> {
    Some(WindowReadback::for_window_op(
        op,
        f64::from(PLACEMENT_TOLERANCE_PX),
        "px",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{OperationReport, OperationTarget, ProcessIdentity};

    fn report(op: &WindowOp) -> OperationReport {
        OperationReport::window_operation(
            OperationTarget::Window {
                id: "w-1".into(),
                title: "Doc".into(),
                process: ProcessIdentity::new(7u32, "boot"),
            },
            op,
            window_readback(op),
        )
    }

    #[test]
    fn geometry_scopes_carry_the_placement_tolerance_in_pixels() {
        for (op, scope) in [
            (
                WindowOp::Resize {
                    width: 3.0,
                    height: 4.0,
                },
                "window_size",
            ),
            (WindowOp::Move { x: 1.0, y: 2.0 }, "window_position"),
        ] {
            let details = report(&op).result.details.unwrap();
            assert_eq!(details["verification_scope"], scope);
            assert_eq!(details["tolerance"], f64::from(PLACEMENT_TOLERANCE_PX));
            assert_eq!(details["units"], "px");
        }
    }
}
