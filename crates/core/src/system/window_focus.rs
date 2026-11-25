use std::time::Duration;

use crate::{
    AdapterError, AppError, DeliverySemantics, ErrorCode, PlatformAdapter, WindowFilter, WindowInfo,
};

const FOCUS_POLL_INTERVAL_MS: u64 = 50;
const FOCUS_CONFIRMATIONS: u8 = 2;

/// Focuses the window and returns it with how far the focus got. A dispatch
/// the adapter reports as sent becomes verified only after this confirmation
/// observes the window focused; one that sent nothing stays not-delivered even
/// when the window is now focused or the confirmation itself fails.
pub(crate) fn focus_and_confirm(
    adapter: &dyn PlatformAdapter,
    window: &WindowInfo,
    lease: &crate::InteractionLease,
) -> Result<(WindowInfo, DeliverySemantics), AppError> {
    let dispatch = adapter.focus_window(window, lease)?;
    let focused = wait_for_focused_window_with_poll_interval(
        adapter,
        &window.id,
        Some(&window.app),
        Duration::from_millis(FOCUS_POLL_INTERVAL_MS),
        lease.deadline(),
    )
    .map_err(|error| stamp_failed_confirmation(error, dispatch))?;
    let disposition = if dispatch == DeliverySemantics::not_delivered() {
        DeliverySemantics::not_delivered()
    } else {
        DeliverySemantics::delivered_verified()
    };
    Ok((focused, disposition))
}

fn stamp_failed_confirmation(error: AppError, dispatch: DeliverySemantics) -> AppError {
    let disposition = if dispatch == DeliverySemantics::not_delivered() {
        DeliverySemantics::not_delivered()
    } else {
        DeliverySemantics::delivered_unverified()
    };
    match error {
        AppError::Adapter(error) => AppError::Adapter(error.with_disposition(disposition)),
        other => other,
    }
}

/// The window when it is already the focused one by `FOCUS_CONFIRMATIONS`
/// consecutive reads, so a focus request has nothing to send.
pub(crate) fn already_focused(
    adapter: &dyn PlatformAdapter,
    window: &WindowInfo,
    deadline: crate::Deadline,
) -> Option<WindowInfo> {
    let mut focused = None;
    for _ in 0..FOCUS_CONFIRMATIONS {
        match observed_focused_window(adapter, Some(&window.app), deadline) {
            Ok(Some(observed)) if observed.id == window.id => focused = Some(observed),
            _ => return None,
        }
    }
    focused
}

/// Where a focus confirmation stands. A focus request lands some time after it
/// is sent, so the wait has two phases that share one deadline: waiting for the
/// requested window to become the focused one, then waiting for it to stay so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusWait {
    AwaitingFocus,
    Stabilizing { confirmed: u8 },
    Settled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusRead {
    OnTarget,
    OffTarget,
}

impl FocusWait {
    pub(crate) fn advance(self, read: FocusRead) -> Self {
        match (self, read) {
            (Self::Settled, _) => Self::Settled,
            (_, FocusRead::OffTarget) => Self::AwaitingFocus,
            (Self::AwaitingFocus, FocusRead::OnTarget) => Self::confirmed(1),
            (Self::Stabilizing { confirmed }, FocusRead::OnTarget) => {
                Self::confirmed(confirmed.saturating_add(1))
            }
        }
    }

    fn confirmed(confirmed: u8) -> Self {
        if confirmed >= FOCUS_CONFIRMATIONS {
            Self::Settled
        } else {
            Self::Stabilizing { confirmed }
        }
    }
}

pub(crate) fn wait_for_focused_window_with_poll_interval(
    adapter: &dyn PlatformAdapter,
    window_id: &str,
    app: Option<&str>,
    poll_interval: Duration,
    deadline: crate::Deadline,
) -> Result<WindowInfo, AppError> {
    let mut state = FocusWait::AwaitingFocus;
    loop {
        let (read, window) = match observed_focused_window(adapter, app, deadline) {
            Ok(Some(window)) if window.id == window_id => (FocusRead::OnTarget, Some(window)),
            Ok(_) => (FocusRead::OffTarget, None),
            Err(AppError::Adapter(error)) if error.permits_retry_by_default() => {
                (FocusRead::OffTarget, None)
            }
            Err(error) => return Err(error),
        };
        state = state.advance(read);
        if let (FocusWait::Settled, Some(window)) = (state, window) {
            return Ok(window);
        }
        if deadline.is_expired() {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Window focus did not settle on the requested window",
            )
            .with_suggestion("Run 'list-windows' to refresh window IDs, then retry.")
            .into());
        }
        if !poll_interval.is_zero() {
            crate::wait::deadline::sleep(poll_interval.min(deadline.remaining()));
        }
    }
}

fn observed_focused_window(
    adapter: &dyn PlatformAdapter,
    app: Option<&str>,
    deadline: crate::Deadline,
) -> Result<Option<WindowInfo>, AppError> {
    match adapter.focused_window(deadline) {
        Ok(window) => Ok(window),
        Err(err) if err.code == ErrorCode::PlatformNotSupported => adapter
            .list_windows(
                &WindowFilter {
                    focused_only: true,
                    app: app.map(str::to_string),
                },
                deadline,
            )
            .map(|windows| windows.into_iter().next())
            .map_err(AppError::Adapter),
        Err(err) => Err(AppError::Adapter(err)),
    }
}

#[cfg(test)]
#[path = "window_focus_tests.rs"]
mod tests;
