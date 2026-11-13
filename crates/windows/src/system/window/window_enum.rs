use agent_desktop_core::{AdapterError, Rect};

/// A top-level window handle, typed for the compiling platform.
///
/// `windows-sys`'s `HWND` is a raw pointer that only exists on Windows; the
/// crate still must compile on the Linux cross-check lane, so the system
/// modules name this alias in signatures and keep the Windows-only calls
/// behind `#[cfg]`. The non-Windows alias stays a raw pointer so
/// `std::ptr::null_mut()` and pointer casts type-check identically.
#[cfg(target_os = "windows")]
pub(crate) type WindowHandle = windows_sys::Win32::Foundation::HWND;
#[cfg(not(target_os = "windows"))]
pub(crate) type WindowHandle = *mut core::ffi::c_void;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Dwm::DWMWA_CLOAKED;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetWindowLongW, GetWindowRect, IsIconic, IsWindowVisible,
    WS_EX_TOOLWINDOW,
};

/// A top-level window as the enumeration pass records it.
///
/// Identity-bearing handle plus the geometry, visibility and ex-style facts
/// the census filter judges - every field read once, off the same HWND, so the
/// filter can cite its own evidence per criterion (A16-1).
#[derive(Debug, Clone, Copy)]
pub(crate) struct EnumeratedWindow {
    pub(crate) handle: WindowHandle,
    pub(crate) visible: bool,
    pub(crate) iconic: bool,
    pub(crate) cloaked: bool,
    pub(crate) tool: bool,
    pub(crate) rect: Rect,
}

impl EnumeratedWindow {
    pub(crate) fn is_zero_sized(&self) -> bool {
        self.rect.width <= 0.0 || self.rect.height <= 0.0
    }
}

/// Enumerates every top-level window on the calling desktop.
///
/// The closure receives each window and stops when it returns `false`, the
/// documented `EnumWindows` contract. `EnumWindows` invokes the callback
/// synchronously on the calling thread, so the visitor is passed by raw
/// pointer through the callback's `lparam` and never crosses threads; the
/// reference is valid for the entire synchronous call.
///
/// The callback runs across an FFI boundary, so the visitor must neither panic
/// (a panic would unwind through an `extern "system"` frame, which is UB) nor
/// re-enter `EnumWindows` or destroy windows while iterating. Every caller in
/// this crate obeys both.
///
/// `EnumWindows` itself reports failure by returning zero, but a visitor that
/// deliberately stops early (returns `false`) also makes it return zero - the
/// same signal as a genuine enumeration failure. The two are told apart here:
/// an empty or partial desktop the visitor chose to stop on stays `Ok`, only a
/// zero return the visitor never asked for is reported as an error.
#[cfg(target_os = "windows")]
pub(crate) fn enumerate_top_level(
    mut visit: impl FnMut(EnumeratedWindow) -> bool,
) -> Result<(), AdapterError> {
    unsafe extern "system" fn callback(window: HWND, lparam: isize) -> i32 {
        let visit = unsafe { &mut *(lparam as *mut Box<dyn FnMut(EnumeratedWindow) -> bool>) };
        let keep_going = visit(EnumeratedWindow {
            handle: window,
            visible: unsafe { IsWindowVisible(window) != 0 },
            iconic: unsafe { IsIconic(window) != 0 },
            cloaked: is_cloaked(window),
            tool: is_tool_window(window),
            rect: window_rect(window),
        });
        i32::from(keep_going)
    }

    let stopped_early = std::rc::Rc::new(std::cell::Cell::new(false));
    let stop_flag = std::rc::Rc::clone(&stopped_early);
    let mut boxed: Box<dyn FnMut(EnumeratedWindow) -> bool> = Box::new(move |window| {
        let keep_going = visit(window);
        if !keep_going {
            stop_flag.set(true);
        }
        keep_going
    });
    let parameter = (&mut boxed as *mut Box<dyn FnMut(EnumeratedWindow) -> bool>) as isize;
    unsafe { windows_sys::Win32::Foundation::SetLastError(0) };
    let succeeded = unsafe { EnumWindows(Some(callback), parameter) };
    if succeeded == 0 && !stopped_early.get() {
        let last_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        return Err(enum_windows_failure(last_error));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn current_station_and_desktop() -> (Option<String>, Option<String>) {
    use windows_sys::Win32::System::StationsAndDesktops::{
        GetProcessWindowStation, GetThreadDesktop,
    };
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;

    let station = unsafe {
        let hwinsta = GetProcessWindowStation();
        user_object_name(hwinsta as _)
    };
    let desktop = unsafe {
        let tid = GetCurrentThreadId();
        let hdesk = GetThreadDesktop(tid);
        user_object_name(hdesk as _)
    };
    (station, desktop)
}

#[cfg(target_os = "windows")]
unsafe fn user_object_name(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<String> {
    use windows_sys::Win32::System::StationsAndDesktops::{GetUserObjectInformationW, UOI_NAME};

    if handle.is_null() {
        return None;
    }
    let mut needed: u32 = 0;
    unsafe {
        GetUserObjectInformationW(handle, UOI_NAME, core::ptr::null_mut(), 0, &mut needed);
    }
    if needed == 0 {
        return None;
    }
    let u16_len = (needed as usize) / core::mem::size_of::<u16>();
    let mut buffer: Vec<u16> = vec![0u16; u16_len];
    let ok = unsafe {
        GetUserObjectInformationW(
            handle,
            UOI_NAME,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    };
    if ok == 0 {
        return None;
    }
    while buffer.last() == Some(&0) {
        buffer.pop();
    }
    String::from_utf16(&buffer).ok()
}

#[cfg(target_os = "windows")]
pub(crate) fn enum_windows_failure(last_error: u32) -> AdapterError {
    let (station, desktop) = current_station_and_desktop();
    enum_windows_failure_with_context(last_error, station, desktop)
}

#[cfg(target_os = "windows")]
pub(crate) fn enum_windows_failure_with_context(
    last_error: u32,
    station: Option<String>,
    desktop: Option<String>,
) -> AdapterError {
    let station_str = station.as_deref().unwrap_or("unknown");
    let desktop_str = desktop.as_deref().unwrap_or("unknown");
    let detail =
        format!("Win32 error: {last_error}; station: {station_str}; desktop: {desktop_str}");

    let is_interactive = station
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("WinSta0"))
        && desktop
            .as_deref()
            .is_some_and(|d| d.eq_ignore_ascii_case("default"));
    let suggestion = if !is_interactive {
        Some(
            "Run agent-desktop in the interactive user desktop session (WinSta0\\default). \
             Background sandboxes or services run in an isolated window station."
                .to_string(),
        )
    } else {
        None
    };

    let mut error = AdapterError::internal("EnumWindows failed to enumerate top-level windows")
        .with_platform_detail(detail)
        .with_details(serde_json::json!({
            "win32_error": last_error,
            "window_station": station,
            "desktop": desktop,
        }));

    if let Some(s) = suggestion {
        error = error.with_suggestion(s);
    }
    error
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn enumerate_top_level(
    _visit: impl FnMut(EnumeratedWindow) -> bool,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Whether the DWM cloaks a window right now.
///
/// Shared because two predicates read the same attribute for different
/// reasons: the enumeration filter excludes cloaked windows from the agent
/// inventory, and the shell-surface liveness predicate reads the cloak as the
/// open/closed signal itself (A26-2 - a dismissed surface survives, cloaked,
/// with `IsWindowVisible` still true).
#[cfg(target_os = "windows")]
pub(crate) fn is_cloaked(window: HWND) -> bool {
    let mut cloaked: u32 = 0;
    let succeeded = unsafe {
        windows_sys::Win32::Graphics::Dwm::DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED as u32,
            (&mut cloaked as *mut u32).cast(),
            core::mem::size_of::<u32>() as u32,
        )
    } == 0;
    succeeded && cloaked != 0
}

#[cfg(target_os = "windows")]
fn is_tool_window(window: HWND) -> bool {
    let ex_style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) };
    (ex_style & WS_EX_TOOLWINDOW as i32) != 0
}

#[cfg(target_os = "windows")]
pub(crate) fn window_rect(window: HWND) -> Rect {
    let mut rect = windows_sys::Win32::Foundation::RECT::default();
    if unsafe { GetWindowRect(window, &mut rect) } == 0 {
        return Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    }
    crate::system::runtime::win_rect::rect_of(&rect)
}

/// Every assertion here drives the live `EnumWindows` bridge, which the
/// non-Windows arm of this module does not have, so the whole module is
/// scoped to the lane that runs it.
#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    /// Both halves of the callback contract: the enumeration keeps calling
    /// back while the visitor answers true, and stops the moment it answers
    /// false.
    ///
    /// The stopping half alone is satisfied by an enumeration that calls back
    /// exactly once and then gives up, so the continuing half is driven
    /// against the same desktop in the same test and the two counts compared.
    #[test]
    fn enumeration_calls_back_for_every_window_and_stops_on_false() {
        let mut visited = Vec::new();
        let mut first = true;

        enumerate_top_level(|window| {
            visited.push(window.handle);
            !std::mem::take(&mut first)
        })
        .expect("enumeration succeeds");

        assert_eq!(
            visited.len(),
            1,
            "the callback returned false after the first window"
        );

        let mut every = Vec::new();
        enumerate_top_level(|window| {
            every.push(window.handle);
            true
        })
        .expect("enumeration succeeds");

        assert!(
            every.len() > visited.len(),
            "a visitor that never stops is called back for more than the one window the stopping visitor saw"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn live_enumeration_observes_the_shell_without_crashing() {
        let mut visible = 0usize;
        let mut total = 0usize;
        enumerate_top_level(|window| {
            total += 1;
            if window.visible && !window.is_zero_sized() {
                visible += 1;
            }
            true
        })
        .expect("live enumeration succeeds");

        assert!(total > 0, "a desktop has at least one top-level window");
        assert!(visible > 0, "the shell is visible");
    }

    #[test]
    fn enum_windows_failure_constructs_diagnostic_context() {
        let err = enum_windows_failure(0);
        assert_eq!(err.code, agent_desktop_core::ErrorCode::Internal);
        assert_eq!(
            err.message,
            "EnumWindows failed to enumerate top-level windows"
        );
        let platform_detail = err.platform_detail.expect("platform detail is present");
        assert!(platform_detail.starts_with("Win32 error: 0; station: "));
        assert!(platform_detail.contains("; desktop: "));

        let details = err.details.expect("details json is present");
        assert_eq!(details.get("win32_error").and_then(|v| v.as_u64()), Some(0));
        assert!(details.get("window_station").is_some());
        assert!(details.get("desktop").is_some());
    }

    #[test]
    fn enum_windows_failure_with_context_formats_and_suggests_correctly() {
        let non_interactive = enum_windows_failure_with_context(
            0,
            Some("WinSta0".to_string()),
            Some("exebox-123".to_string()),
        );
        assert_eq!(
            non_interactive.platform_detail.as_deref(),
            Some("Win32 error: 0; station: WinSta0; desktop: exebox-123")
        );
        assert!(non_interactive.suggestion.is_some());
        assert!(
            non_interactive
                .suggestion
                .as_deref()
                .unwrap()
                .contains("WinSta0\\default")
        );

        let interactive = enum_windows_failure_with_context(
            0,
            Some("WinSta0".to_string()),
            Some("default".to_string()),
        );
        assert_eq!(
            interactive.platform_detail.as_deref(),
            Some("Win32 error: 0; station: WinSta0; desktop: default")
        );
        assert!(interactive.suggestion.is_none());

        let interactive_ci = enum_windows_failure_with_context(
            0,
            Some("winsta0".to_string()),
            Some("Default".to_string()),
        );
        assert!(interactive_ci.suggestion.is_none());

        let unknown = enum_windows_failure_with_context(5, None, None);
        assert_eq!(
            unknown.platform_detail.as_deref(),
            Some("Win32 error: 5; station: unknown; desktop: unknown")
        );
        assert!(unknown.suggestion.is_some());
    }
}

/// How long a window gets to answer the liveness ping before a caller treats
/// it as unable to service window messages.
const PUMP_PROBE_MS: u64 = 500;

/// Whether the window's owning thread is dispatching messages right now.
///
/// Callers that are about to reach that thread's message queue ask this first,
/// because those calls block inside the OS when the thread never dispatches
/// and no `Deadline` can interrupt one already in flight. The wording of the
/// refusal is the caller's - activation and window operations report the same
/// condition with different envelopes - so only the probe is shared.
pub(crate) fn window_is_responsive(handle: WindowHandle) -> bool {
    #[cfg(target_os = "windows")]
    {
        crate::tree::automation::window_is_pumping(handle as isize, PUMP_PROBE_MS)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        true
    }
}
