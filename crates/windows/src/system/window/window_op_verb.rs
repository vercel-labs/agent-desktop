use agent_desktop_core::InteractionPolicy;

const SW_RESTORE: i32 = 9;
const SW_SHOWNOACTIVATE: i32 = 4;

/// The show command that un-minimizes. `SW_RESTORE` activates the window and
/// brings it to the top; `SW_SHOWNOACTIVATE` un-minimizes it with the
/// foreground left alone, so only a policy that lets a window take focus may
/// use the first.
pub(crate) fn restore_command(policy: InteractionPolicy) -> i32 {
    if policy.allow_focus_steal {
        SW_RESTORE
    } else {
        SW_SHOWNOACTIVATE
    }
}

/// Whether an un-minimize must put the window under the user's foreground
/// window: `SW_SHOWNOACTIVATE` alone leaves it at the bottom of the order.
pub(crate) fn restore_goes_below_foreground(verb: ShowVerb, policy: InteractionPolicy) -> bool {
    matches!(verb, ShowVerb::Restore) && !policy.allow_focus_steal
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
