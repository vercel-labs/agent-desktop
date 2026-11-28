use agent_desktop_core::InteractionPolicy;

const SW_RESTORE: i32 = 9;
const SW_SHOWNOACTIVATE: i32 = 4;

/// The show command of a restore. `SW_RESTORE` activates the window, so only a
/// policy that lets a window take focus uses it; `SW_SHOWNOACTIVATE` returns a
/// maximized window to its normal size with the foreground left alone.
pub(crate) fn restore_command(policy: InteractionPolicy) -> i32 {
    if policy.allow_focus_steal {
        SW_RESTORE
    } else {
        SW_SHOWNOACTIVATE
    }
}

/// A restore of a minimized window needs a policy that lets a window take the
/// focus: applications such as the Microsoft Management Console and Character
/// Map activate themselves while they are restored whatever show command is
/// used, so no quiet un-minimize exists. A window that is not minimized, a
/// maximized one coming back to its normal size, restores headless.
pub(crate) fn restore_permitted(
    minimized: bool,
    policy: InteractionPolicy,
) -> Result<(), agent_desktop_core::AdapterError> {
    if minimized {
        agent_desktop_core::WindowOp::require_unminimize_allowed(policy)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum ShowVerb {
    Minimize,
    Maximize,
    Restore,
}

/// Whether the placement the window reports satisfies the verb that was asked.
///
/// Restore is "no longer minimized" rather than an exact normal placement: a
/// window minimized while maximized carries `WPF_RESTORETOMAXIMIZED`, so
/// `SW_RESTORE` correctly returns it to maximized and demanding
/// `SW_SHOWNORMAL` would call that ordinary sequence a failure. The product
/// verb undoes the minimize; it does not promise to un-maximize.
#[cfg(target_os = "windows")]
pub(crate) fn verb_is_satisfied(verb: ShowVerb, show_cmd: u32) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED};

    match verb {
        ShowVerb::Minimize => show_cmd == SW_SHOWMINIMIZED as u32,
        ShowVerb::Maximize => show_cmd == SW_SHOWMAXIMIZED as u32,
        ShowVerb::Restore => show_cmd != SW_SHOWMINIMIZED as u32,
    }
}

#[cfg(test)]
#[path = "window_op_verb_tests.rs"]
mod tests;
