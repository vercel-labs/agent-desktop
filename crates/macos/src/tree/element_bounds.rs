use agent_desktop_core::{AdapterError, Deadline, ErrorCode, Rect};

use super::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn rect_from_parts(
    point: core_graphics::geometry::CGPoint,
    size: core_graphics::geometry::CGSize,
) -> Option<Rect> {
    Rect {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    }
    .validate()
    .ok()
}

#[cfg(target_os = "macos")]
pub(crate) fn read_bounds_with_deadline(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<Rect>, agent_desktop_core::AdapterError> {
    let point = super::resolve::resolve_ax_read::read_point(element, "AXPosition", deadline)?;
    let size = super::resolve::resolve_ax_read::read_size(element, "AXSize", deadline)?;
    Ok(point
        .zip(size)
        .and_then(|(point, size)| rect_from_parts(point, size)))
}

pub(crate) fn owning_window_bounds(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    crate::tree::properties::attributes::set_messaging_timeout(element, deadline)?;
    let window = first_owning_container(|attribute| {
        crate::tree::properties::attributes::copy_element_attr_result(element, attribute, deadline)
    })
    .map_err(|(attribute, error)| read_error(attribute, error))?;
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    let Some(window) = window else {
        return Ok(None);
    };
    crate::tree::element_bounds::read_bounds_with_deadline(&window, deadline)
}

pub(crate) fn clips_descendants(role: &str) -> bool {
    matches!(
        role,
        "AXWindow" | "AXScrollArea" | "AXWebArea" | "AXSheet" | "AXPopover"
    )
}

pub(crate) fn intersect_rects(left: Rect, right: Rect) -> Option<Rect> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let width = (left.x + left.width).min(right.x + right.width) - x;
    let height = (left.y + left.height).min(right.y + right.height) - y;
    (width > 0.0 && height > 0.0).then_some(Rect {
        x,
        y,
        width,
        height,
    })
}

/// Whether an element with this role is drawn on a surface of its own. The
/// accessibility tree hangs a popover below the control it is anchored to, in
/// that control's window, but the popover can reach past that window, so its
/// content is clipped by the popover and by nothing above it. A popover is
/// named by its role or by its subrole, the same test the popover surface is
/// found by.
pub(crate) fn owns_surface(role: &str, subrole: Option<&str>) -> bool {
    role == "AXPopover" || subrole == Some("AXPopover")
}

pub(crate) fn clipped_viewport(
    parent: Option<Rect>,
    role: &str,
    subrole: Option<&str>,
    bounds: Option<Rect>,
) -> Option<Rect> {
    if owns_surface(role, subrole) {
        return bounds;
    }
    if !clips_descendants(role) {
        return parent;
    }
    narrowed(parent, bounds)
}

fn narrowed(viewport: Option<Rect>, bounds: Option<Rect>) -> Option<Rect> {
    let viewport = viewport?;
    let bounds = bounds?;
    Some(intersect_rects(viewport, bounds).unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    }))
}

/// The viewport of an element whose nearest surface is `surface` and whose
/// clipping ancestors below that surface have the bounds in `clips`.
fn within(surface: Option<Rect>, clips: &[Option<Rect>]) -> Option<Rect> {
    clips
        .iter()
        .fold(surface, |viewport, bounds| narrowed(viewport, *bounds))
}

/// The viewport inside an `AXWindow` ancestor. The element's own `AXWindow`
/// attribute names that window when it answers; an element drawn in a sheet has
/// no such value, so the ancestor the parent walk reached is the owning window.
fn window_viewport(
    attribute_window: Option<Rect>,
    ancestor_window: Option<Rect>,
    clips: &[Option<Rect>],
) -> Option<Rect> {
    within(attribute_window.or(ancestor_window), clips)
}

pub(crate) fn ancestor_viewport(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    let window = owning_window_bounds(element, deadline)?;
    let instant = deadline;
    let mut clips = Vec::new();
    let mut current = super::surface_read::element(element, "AXParent", instant)?;
    for _ in 0..super::properties::element::ABSOLUTE_MAX_DEPTH {
        let Some(parent) = current else {
            return Ok(within(window, &clips));
        };
        let Some(role) = super::surface_read::string(&parent, "AXRole", instant)? else {
            return Ok(None);
        };
        if role == "AXApplication" {
            return Ok(within(window, &clips));
        }
        let subrole = if owns_surface(&role, None) {
            None
        } else {
            super::surface_read::string(&parent, "AXSubrole", instant)?
        };
        if owns_surface(&role, subrole.as_deref()) {
            let surface = read_bounds_with_deadline(&parent, instant)?;
            return Ok(within(surface, &clips));
        }
        let own_bounds = if clips_descendants(&role) {
            let bounds = read_bounds_with_deadline(&parent, instant)?;
            clips.push(bounds);
            bounds
        } else {
            None
        };
        if role == "AXWindow" {
            return Ok(window_viewport(window, own_bounds, &clips));
        }
        current = super::surface_read::element(&parent, "AXParent", instant)?;
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Clipping ancestry exceeded its bounded depth",
    ))
}

/// The viewport an element is clipped by. `AXTopLevelUIElement` is not a
/// substitute: for menu content it resolves to the menu bar, a 29-point strip
/// that reports every open menu item as offscreen. An element with no window is
/// drawn on its own surface, so its clipping viewport is simply unknown.
fn first_owning_container(
    mut read: impl FnMut(&'static str) -> Result<Option<AXElement>, i32>,
) -> Result<Option<AXElement>, (&'static str, i32)> {
    read("AXWindow").map_err(|error| ("AXWindow", error))
}

fn read_error(attribute: &str, error: i32) -> AdapterError {
    AdapterError::new(
        if error == accessibility_sys::kAXErrorCannotComplete {
            ErrorCode::Timeout
        } else if error == accessibility_sys::kAXErrorAPIDisabled {
            ErrorCode::PermDenied
        } else if error == accessibility_sys::kAXErrorInvalidUIElement {
            ErrorCode::StaleRef
        } else {
            ErrorCode::ActionFailed
        },
        format!("Could not read {attribute} for live state"),
    )
    .with_details(serde_json::json!({ "attribute": attribute, "ax_error": error }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn native_bounds_decoder_rejects_invalid_geometry_before_live_or_snapshot_use() {
        use core_graphics::geometry::{CGPoint, CGSize};
        for (point, size) in [
            (CGPoint::new(0.0, 33.0), CGSize::new(-1.0, 0.0)),
            (CGPoint::new(0.0, 0.0), CGSize::new(10.0, -0.5)),
            (CGPoint::new(f64::NAN, 0.0), CGSize::new(10.0, 10.0)),
            (CGPoint::new(0.0, 0.0), CGSize::new(f64::INFINITY, 10.0)),
            (CGPoint::new(0.0, 0.0), CGSize::new(f64::MAX, 10.0)),
        ] {
            assert_eq!(rect_from_parts(point, size), None);
        }
        for size in [CGSize::new(0.0, 0.0), CGSize::new(20.0, 10.0)] {
            let bounds = rect_from_parts(CGPoint::new(-20.0, 33.0), size).unwrap();
            assert_eq!(bounds.x, -20.0);
            assert_eq!(bounds.width, size.width);
            assert_eq!(bounds.height, size.height);
        }
    }

    fn rect(x: f64, width: f64) -> Rect {
        Rect {
            x,
            y: 0.0,
            width,
            height: 100.0,
        }
    }

    #[test]
    fn only_clipping_containers_narrow_the_viewport() {
        let window = Some(rect(0.0, 500.0));
        assert_eq!(
            clipped_viewport(window, "AXGroup", None, Some(rect(20.0, 10.0))),
            window
        );
        assert_eq!(
            clipped_viewport(window, "AXScrollArea", None, Some(rect(20.0, 100.0))),
            Some(rect(20.0, 100.0))
        );
    }

    #[test]
    fn disjoint_viewports_remain_known_empty_through_descendants() {
        let empty = clipped_viewport(
            Some(rect(0.0, 100.0)),
            "AXScrollArea",
            None,
            Some(rect(200.0, 100.0)),
        );
        assert_eq!(
            crate::tree::state_reader::offscreen(Some(rect(-50.0, 500.0)), empty),
            Some(true)
        );
        let nested = clipped_viewport(empty, "AXScrollArea", None, Some(rect(-50.0, 500.0)));
        assert_eq!(
            crate::tree::state_reader::offscreen(Some(rect(-50.0, 500.0)), nested),
            Some(true)
        );
    }

    #[test]
    fn a_popover_is_its_own_viewport_even_outside_the_window_it_hangs_in() {
        let window = Some(rect(0.0, 100.0));
        let popover = Some(rect(300.0, 80.0));

        let viewport = clipped_viewport(window, "AXPopover", None, popover);

        assert_eq!(viewport, popover);
        assert_eq!(
            crate::tree::state_reader::offscreen(Some(rect(310.0, 20.0)), viewport),
            Some(false)
        );
        assert_eq!(clipped_viewport(None, "AXPopover", None, popover), popover);
        assert_eq!(clipped_viewport(window, "AXPopover", None, None), None);
    }

    #[test]
    fn a_popover_named_by_subrole_owns_its_surface() {
        let popover = Some(rect(400.0, 300.0));
        assert_eq!(
            clipped_viewport(
                Some(rect(0.0, 500.0)),
                "AXGroup",
                Some("AXPopover"),
                popover
            ),
            popover
        );
    }

    #[test]
    fn clips_below_a_surface_narrow_that_surface_and_nothing_above_it() {
        let popover = Some(rect(300.0, 80.0));
        let scroll_area = Some(rect(320.0, 30.0));

        assert_eq!(within(popover, &[scroll_area]), scroll_area);
        assert_eq!(within(popover, &[]), popover);
        assert_eq!(within(popover, &[None]), None);
    }

    #[test]
    fn an_element_with_no_window_attribute_is_clipped_by_its_window_ancestor() {
        let window = Some(rect(100.0, 600.0));
        let sheet = Some(rect(50.0, 800.0));
        assert_eq!(window_viewport(None, window, &[sheet, window]), window);
        assert_eq!(window_viewport(None, None, &[sheet]), None);
    }

    #[test]
    fn the_window_attribute_wins_over_the_ancestor_window() {
        let attributed = Some(rect(100.0, 600.0));
        let ancestor = Some(rect(0.0, 50.0));
        assert_eq!(window_viewport(attributed, ancestor, &[]), attributed);
    }

    #[test]
    fn missing_clipping_geometry_is_unknown_not_empty() {
        assert_eq!(
            clipped_viewport(Some(rect(0.0, 100.0)), "AXScrollArea", None, None),
            None
        );
        assert_eq!(
            clipped_viewport(None, "AXScrollArea", None, Some(rect(0.0, 100.0))),
            None
        );
    }
    /// A menu item's `AXTopLevelUIElement` is the menu bar, whose 29-point height
    /// clips every open menu item. A window-less element therefore reports no
    /// clipping viewport rather than a wrong one.
    #[test]
    fn a_window_less_element_reports_no_clipping_viewport() {
        let mut attributes = Vec::new();
        let container = first_owning_container(|attribute| {
            attributes.push(attribute);
            Ok(None)
        })
        .unwrap();

        assert!(container.is_none());
        assert_eq!(attributes, ["AXWindow"]);
    }

    #[test]
    fn incomplete_window_read_never_falls_through_to_a_weaker_container() {
        for error in [
            accessibility_sys::kAXErrorCannotComplete,
            accessibility_sys::kAXErrorInvalidUIElement,
        ] {
            let calls = std::cell::Cell::new(0);
            let result = first_owning_container(|_| {
                calls.set(calls.get() + 1);
                Err(error)
            });

            let failure = match result {
                Err(failure) => failure,
                Ok(_) => panic!("incomplete AXWindow read must fail"),
            };
            assert_eq!(failure, ("AXWindow", error));
            assert_eq!(calls.get(), 1);
        }
    }
}
