//! Provider-side activation when Windows refuses a background caller's raise.
//!
//! Reapplying the current visual state lets the window provider activate its
//! own window without changing a maximized window to normal, injecting keys,
//! or sharing input queues. Providers may decline; the caller still verifies
//! the exact foreground HWND and its stored process identity.

use agent_desktop_core::{AdapterError, Deadline};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationWindowPattern, UIA_E_TIMEOUT,
    UIA_WindowPatternId, WindowVisualState_Minimized,
};
use windows::core::Interface;

use super::window_enum::WindowHandle;
use super::window_identity::WindowIdentityEvidence;

const PROVIDER_TIMEOUT_MS: u32 = 150;

pub(super) fn activate(
    handle: WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if !evidence.owns_handle_now()? {
        return Err(super::window_activate::recycled_before_foreground());
    }
    let Ok((client, pattern, state)) = window_pattern(handle, deadline) else {
        return Ok(());
    };
    if deadline.is_expired() || state == WindowVisualState_Minimized {
        return Ok(());
    }
    if !evidence.owns_handle_now()? {
        return Err(super::window_activate::recycled_before_foreground());
    }
    if configure_timeout(&client, deadline).is_err() {
        return Ok(());
    }
    unsafe {
        let _ = pattern.SetWindowVisualState(state);
    }
    Ok(())
}

fn window_pattern(
    handle: WindowHandle,
    deadline: Deadline,
) -> windows::core::Result<(
    IUIAutomation2,
    IUIAutomationWindowPattern,
    windows::Win32::UI::Accessibility::WindowVisualState,
)> {
    unsafe {
        let bounded: IUIAutomation2 = CoCreateInstance(&CUIAutomation8, None, CLSCTX_ALL)?;
        bounded.SetAutoSetFocus(true)?;
        let client: IUIAutomation = bounded.cast()?;
        configure_timeout(&bounded, deadline)?;
        let element = client.ElementFromHandle(HWND(handle))?;
        configure_timeout(&bounded, deadline)?;
        let pattern: IUIAutomationWindowPattern =
            element.GetCurrentPatternAs(UIA_WindowPatternId)?;
        configure_timeout(&bounded, deadline)?;
        let state = pattern.CurrentWindowVisualState()?;
        Ok((bounded, pattern, state))
    }
}

fn configure_timeout(client: &IUIAutomation2, deadline: Deadline) -> windows::core::Result<()> {
    let timeout = deadline.remaining_ms().min(u64::from(PROVIDER_TIMEOUT_MS)) as u32;
    if timeout == 0 {
        return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
            UIA_E_TIMEOUT as i32,
        )));
    }
    unsafe {
        client.SetConnectionTimeout(timeout)?;
        client.SetTransactionTimeout(timeout)
    }
}

#[cfg(test)]
#[path = "window_activate_uia_tests.rs"]
mod tests;
