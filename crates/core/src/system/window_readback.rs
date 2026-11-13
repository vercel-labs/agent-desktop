use serde_json::{Value, json};

use crate::WindowOp;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowReadback {
    scope: &'static str,
    tolerance: Option<(f64, &'static str)>,
}

impl WindowReadback {
    pub const fn state(scope: &'static str) -> Self {
        Self {
            scope,
            tolerance: None,
        }
    }

    pub const fn geometry(scope: &'static str, tolerance: f64, units: &'static str) -> Self {
        Self {
            scope,
            tolerance: Some((tolerance, units)),
        }
    }

    pub fn for_window_op(op: &WindowOp, tolerance: f64, units: &'static str) -> Self {
        match op {
            WindowOp::Resize { .. } => Self::geometry("window_size", tolerance, units),
            WindowOp::Move { .. } => Self::geometry("window_position", tolerance, units),
            WindowOp::Minimize | WindowOp::Restore => Self::state("window_minimized_state"),
            WindowOp::Maximize => Self::state("window_maximized_state"),
        }
    }

    pub(crate) fn details(&self) -> Value {
        match self.tolerance {
            Some((tolerance, units)) => json!({
                "verification_scope": self.scope,
                "tolerance": tolerance,
                "units": units,
            }),
            None => json!({ "verification_scope": self.scope }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_window_operation_names_the_scope_it_reads_back() {
        let scope = |op: WindowOp| {
            WindowReadback::for_window_op(&op, 2.0, "px").details()["verification_scope"].clone()
        };
        assert_eq!(scope(WindowOp::Move { x: 1.0, y: 2.0 }), "window_position");
        assert_eq!(
            scope(WindowOp::Resize {
                width: 3.0,
                height: 4.0
            }),
            "window_size"
        );
        assert_eq!(scope(WindowOp::Minimize), "window_minimized_state");
        assert_eq!(scope(WindowOp::Restore), "window_minimized_state");
        assert_eq!(scope(WindowOp::Maximize), "window_maximized_state");
    }

    #[test]
    fn only_geometry_carries_a_tolerance() {
        let geometry = WindowReadback::for_window_op(&WindowOp::Move { x: 0.0, y: 0.0 }, 2.0, "px");
        let state = WindowReadback::for_window_op(&WindowOp::Maximize, 2.0, "px");
        assert_eq!(geometry.details()["tolerance"], 2.0);
        assert!(state.details().get("tolerance").is_none());
    }
}
