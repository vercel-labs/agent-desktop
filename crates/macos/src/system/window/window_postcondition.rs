use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Rect};

pub(crate) const TOLERANCE: f64 = 2.0;

pub(crate) fn wait_for_geometry(
    element: &crate::tree::AXElement,
    expected: Rect,
    position: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        let bounds = crate::tree::element_bounds::read_bounds_with_deadline(element, deadline)?;
        Ok(bounds.is_some_and(|actual| geometry_matches(actual, expected, position)))
    })
}

pub(crate) fn wait_for_minimized(
    element: &crate::tree::AXElement,
    expected: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        Ok(crate::tree::surface_read::boolean(element, "AXMinimized", deadline)? == Some(expected))
    })
}

fn wait_until(
    deadline: Deadline,
    complete: impl FnMut() -> Result<bool, AdapterError>,
) -> Result<(), AdapterError> {
    let step = std::time::Duration::from_millis(10);
    let reached = deadline
        .poll_until(deadline.remaining(), step, complete, |read| match read {
            Ok(reached) => *reached,
            Err(error) => !error.is_transient_incomplete_read(),
        })
        .map_err(after_delivery)?;
    if reached {
        return Ok(());
    }
    Err(after_delivery(AdapterError::timeout(
        "Window operation did not reach its requested postcondition",
    )))
}

fn geometry_matches(actual: Rect, expected: Rect, position: bool) -> bool {
    let values = if position {
        [(actual.x, expected.x), (actual.y, expected.y)]
    } else {
        [
            (actual.width, expected.width),
            (actual.height, expected.height),
        ]
    };
    values
        .into_iter()
        .all(|(actual, expected)| (actual - expected).abs() <= TOLERANCE)
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
mod tests {
    use super::*;
    use accessibility_sys::{kAXErrorAPIDisabled, kAXErrorCannotComplete};

    fn generous_deadline() -> Deadline {
        Deadline::after(5_000).expect("deadline")
    }

    #[test]
    fn transient_read_errors_do_not_end_the_postcondition_poll() {
        let mut failures_left = 2;
        let mut reads = 0;
        let result = wait_until(generous_deadline(), || {
            reads += 1;
            if failures_left > 0 {
                failures_left -= 1;
                return Err(crate::tree::surface_read::map_error(
                    "AXMinimized",
                    kAXErrorCannotComplete,
                ));
            }
            Ok(true)
        });

        assert!(result.is_ok());
        assert_eq!(reads, 3);
    }

    #[test]
    fn terminal_read_errors_end_the_postcondition_poll_at_once() {
        let mut reads = 0;
        let error = wait_until(generous_deadline(), || {
            reads += 1;
            Err(crate::tree::surface_read::map_error(
                "AXMinimized",
                kAXErrorAPIDisabled,
            ))
        })
        .expect_err("terminal error");

        assert_eq!(reads, 1);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    }

    #[test]
    fn geometry_verification_tolerates_ax_rounding_only() {
        let expected = Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
        };
        let rounded = Rect {
            x: 11.0,
            y: 19.0,
            width: 99.0,
            height: 81.0,
        };
        let wrong = Rect {
            width: 90.0,
            ..rounded
        };

        assert!(geometry_matches(rounded, expected, true));
        assert!(geometry_matches(rounded, expected, false));
        assert!(!geometry_matches(wrong, expected, false));
    }
}
