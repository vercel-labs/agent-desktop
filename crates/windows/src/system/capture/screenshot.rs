//! Screenshot orchestration: target dispatch, identity sandwich, precedence.

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, DisplayInfo, ErrorCode, ImageBuffer,
    ScreenshotTarget, WindowInfo,
};

use super::capture_backend::{CaptureSubject, capture_with_precedence};
use crate::system::display::{
    capture_selection, display_at, scale_for_bounds, verify_display_identity,
};
use crate::system::window::window_ops::parse_handle;
use crate::system::window::window_resolve::resolve_window_strict;

pub(crate) fn screenshot(
    target: ScreenshotTarget,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    dispatch_target(target, deadline).map_err(not_delivered)
}

fn dispatch_target(
    target: ScreenshotTarget,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    deadline.ensure_remaining()?;
    match target {
        ScreenshotTarget::Screen(index) => capture_screen(index, deadline),
        ScreenshotTarget::Display { index, expected } => {
            capture_display(index, &expected, deadline)
        }
        ScreenshotTarget::ExactWindow(window) => capture_window(&window, deadline),
        ScreenshotTarget::FullScreen => capture_screen(0, deadline),
    }
}

fn not_delivered(error: AdapterError) -> AdapterError {
    if error.disposition == DeliverySemantics::Unknown {
        error.with_disposition(DeliverySemantics::not_delivered())
    } else {
        error
    }
}

pub(crate) fn capture_screen(
    index: usize,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    deadline.ensure_remaining()?;
    let expected = display_at(index, deadline)?;
    capture_display(index, &expected, deadline)
}

pub(crate) fn capture_display(
    index: usize,
    expected: &DisplayInfo,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    deadline.ensure_remaining()?;
    let (raw_index, current) = capture_selection(expected, deadline)?;
    deadline.ensure_remaining()?;
    verify_display_identity(index, expected, &current)?;
    let captured = capture_with_precedence(CaptureSubject::Display { index: raw_index }, deadline)?;
    deadline.ensure_remaining()?;
    let after = display_at(raw_index, deadline)?;
    deadline.ensure_remaining()?;
    verify_display_identity(index, &current, &after)?;
    Ok(captured)
}

pub(crate) fn capture_window(
    window: &WindowInfo,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    if window.process_instance.is_none() {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Exact window screenshot requires a process instance token",
        )
        .with_suggestion("Refresh the target with 'list-windows', then retry")
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    deadline.ensure_remaining()?;
    let verified = resolve_window_strict(window, deadline)?;
    let handle = parse_handle(&verified.id);
    if handle.is_null() {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "window id is not a Windows HWND-shaped id",
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    let scale = scale_for_bounds(verified.bounds, deadline)?;
    deadline.ensure_remaining()?;
    let captured = capture_with_precedence(
        CaptureSubject::Window {
            handle,
            scale_factor: scale,
        },
        deadline,
    )?;
    deadline.ensure_remaining()?;
    resolve_window_strict(&verified, deadline)?;
    Ok(captured)
}

#[cfg(test)]
pub(crate) fn coerce_screenshot_disposition_for_test(error: AdapterError) -> AdapterError {
    not_delivered(error)
}

#[cfg(all(test, target_os = "windows"))]
#[path = "screenshot_tests.rs"]
mod tests;
