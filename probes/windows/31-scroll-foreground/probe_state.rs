//! Foreground, focus, activation, z-order and minimized readings for one trial.
//!
//! Every reading is a plain Win32 or UI Automation read. Window titles are never
//! recorded; a window is described by handle, process id and class name.

use serde_json::{Value, json};
use uiautomation::UIAutomation;
use uiautomation::patterns::UIScrollPattern;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Dwm::DwmGetWindowAttribute;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GUITHREADINFO, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo,
    GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
};

const DWMWA_CLOAKED: u32 = 14;
const SLOW_PERCENT_READ_MS: u128 = 250;

pub fn raw(handle: isize) -> HWND {
    handle as HWND
}

pub fn window_pid(handle: isize) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(raw(handle), &mut pid) };
    pid
}

pub fn window_thread(handle: isize) -> u32 {
    unsafe { GetWindowThreadProcessId(raw(handle), std::ptr::null_mut()) }
}

pub fn class_of(handle: isize) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(raw(handle), buffer.as_mut_ptr(), 256) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

pub fn title_of(handle: isize) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(raw(handle), buffer.as_mut_ptr(), 512) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

pub fn rect_of(handle: isize) -> Option<[i32; 4]> {
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(raw(handle), &mut rect) } == 0 {
        return None;
    }
    Some([rect.left, rect.top, rect.right, rect.bottom])
}

pub fn is_iconic(handle: isize) -> bool {
    unsafe { IsIconic(raw(handle)) != 0 }
}

pub fn is_visible(handle: isize) -> bool {
    unsafe { IsWindowVisible(raw(handle)) != 0 }
}

pub fn is_window(handle: isize) -> bool {
    handle != 0 && unsafe { IsWindow(raw(handle)) != 0 }
}

fn cloaked(handle: isize) -> bool {
    let mut value: i32 = 0;
    let result = unsafe {
        DwmGetWindowAttribute(
            raw(handle),
            DWMWA_CLOAKED,
            &mut value as *mut i32 as *mut _,
            std::mem::size_of::<i32>() as u32,
        )
    };
    result == 0 && value != 0
}

unsafe extern "system" fn collect(handle: HWND, out: LPARAM) -> i32 {
    let list = unsafe { &mut *(out as *mut Vec<isize>) };
    list.push(handle as isize);
    1
}

/// Every top-level window, in the z-order `EnumWindows` reports (top first).
pub fn top_level_windows() -> Vec<isize> {
    let mut list: Vec<isize> = Vec::new();
    unsafe { EnumWindows(Some(collect), &mut list as *mut Vec<isize> as LPARAM) };
    list
}

fn shown(handle: isize) -> bool {
    is_visible(handle) && !cloaked(handle)
}

fn thread_info(thread: u32) -> (isize, isize) {
    let mut info: GUITHREADINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(thread, &mut info) } == 0 {
        return (0, 0);
    }
    (info.hwndActive as isize, info.hwndFocus as isize)
}

pub fn foreground() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub fg: isize,
    pub fg_pid: u32,
    pub fg_active: isize,
    pub fg_focus: isize,
    pub f_active: isize,
    pub f_focus: isize,
    pub t_active: isize,
    pub t_focus: isize,
    pub uia_focus_rid: Option<Vec<i32>>,
    pub uia_focus_pid: Option<u32>,
    pub t_iconic: bool,
    pub t_visible: bool,
    pub t_above_f: bool,
    pub z_all: i32,
    pub z_shown: i32,
    pub above_t: Vec<isize>,
    pub v_percent: Option<f64>,
}

pub struct Watch<'a> {
    pub automation: &'a UIAutomation,
    pub f: isize,
    pub t: isize,
    pub scroll: Option<&'a UIScrollPattern>,
}

/// Whether the scroll percent is still read in this trial. A read that blocks
/// (a suspended or minimized provider) is read once and then carried forward,
/// so one slow provider cannot starve the foreground polling.
pub struct PercentReader {
    pub enabled: bool,
    pub last: Option<f64>,
    pub slow_reads: u32,
    pub failed_reads: u32,
}

impl PercentReader {
    pub fn new() -> Self {
        Self {
            enabled: true,
            last: None,
            slow_reads: 0,
            failed_reads: 0,
        }
    }

    fn read(&mut self, scroll: Option<&UIScrollPattern>) -> Option<f64> {
        let pattern = scroll?;
        if !self.enabled {
            return self.last;
        }
        let started = std::time::Instant::now();
        let value = pattern.get_vertical_scroll_percent().ok();
        if started.elapsed().as_millis() > SLOW_PERCENT_READ_MS {
            self.slow_reads += 1;
            self.enabled = false;
        }
        if value.is_none() {
            self.failed_reads += 1;
            if self.failed_reads >= 3 {
                self.enabled = false;
            }
            return self.last;
        }
        self.last = value;
        value
    }
}

pub fn read(watch: &Watch<'_>, percent: &mut PercentReader) -> State {
    let fg = foreground();
    let (fg_active, fg_focus) = thread_info(0);
    let (f_active, f_focus) = thread_info(window_thread(watch.f));
    let (t_active, t_focus) = thread_info(window_thread(watch.t));
    let focused = watch.automation.get_focused_element().ok();
    let uia_focus_rid = focused.as_ref().and_then(|element| element.get_runtime_id().ok());
    let uia_focus_pid = focused.as_ref().and_then(|element| element.get_process_id().ok());
    let all = top_level_windows();
    let z_all = all.iter().position(|handle| *handle == watch.t).map_or(-1, |index| index as i32);
    let z_f = all.iter().position(|handle| *handle == watch.f).map_or(-1, |index| index as i32);
    let shown_windows: Vec<isize> = all.iter().copied().filter(|handle| shown(*handle)).collect();
    let z_shown = shown_windows
        .iter()
        .position(|handle| *handle == watch.t)
        .map_or(-1, |index| index as i32);
    let above_t = match z_shown {
        index if index > 0 => shown_windows[..index as usize].to_vec(),
        _ => Vec::new(),
    };
    State {
        fg,
        fg_pid: window_pid(fg),
        fg_active,
        fg_focus,
        f_active,
        f_focus,
        t_active,
        t_focus,
        uia_focus_rid,
        uia_focus_pid,
        t_iconic: is_iconic(watch.t),
        t_visible: is_visible(watch.t),
        t_above_f: z_all >= 0 && z_f >= 0 && z_all < z_f,
        z_all,
        z_shown,
        above_t,
        v_percent: percent.read(watch.scroll),
    }
}

impl State {
    /// The fields that must stop changing before a trial is read as settled.
    /// Raw z indexes are left out: an unrelated tooltip shifts them.
    pub fn settled_with(&self, other: &State) -> bool {
        self.changed_fields(other).is_empty()
    }

    /// Names of the judged fields that differ, plus `raised_past` when the
    /// target moved above a shown window that was above it before.
    pub fn changed_fields(&self, later: &State) -> Vec<&'static str> {
        let mut fields = Vec::new();
        let mut check = |name: &'static str, differs: bool| {
            if differs {
                fields.push(name);
            }
        };
        check("fg", self.fg != later.fg);
        check("fg_active", self.fg_active != later.fg_active);
        check("fg_focus", self.fg_focus != later.fg_focus);
        check("f_active", self.f_active != later.f_active);
        check("f_focus", self.f_focus != later.f_focus);
        check("t_active", self.t_active != later.t_active);
        check("t_focus", self.t_focus != later.t_focus);
        check(
            "uia_focus",
            self.uia_focus_rid != later.uia_focus_rid || self.uia_focus_pid != later.uia_focus_pid,
        );
        check("t_iconic", self.t_iconic != later.t_iconic);
        check("t_visible", self.t_visible != later.t_visible);
        check("t_above_f", self.t_above_f != later.t_above_f);
        check("raised_past", self.raised_past(later) > 0);
        check("v_percent", self.v_percent != later.v_percent);
        fields
    }

    /// How many shown windows that were above the target are now below it.
    pub fn raised_past(&self, later: &State) -> usize {
        self.above_t
            .iter()
            .filter(|handle| is_window(**handle) && shown(**handle) && !later.above_t.contains(*handle))
            .count()
    }

    pub fn to_json(&self) -> Value {
        json!({
            "fg": self.fg,
            "fg_pid": self.fg_pid,
            "fg_class": class_of(self.fg),
            "fg_thread_active": self.fg_active,
            "fg_thread_focus": self.fg_focus,
            "f_thread_active": self.f_active,
            "f_thread_focus": self.f_focus,
            "t_thread_active": self.t_active,
            "t_thread_focus": self.t_focus,
            "uia_focus_runtime_id": self.uia_focus_rid,
            "uia_focus_pid": self.uia_focus_pid,
            "t_iconic": self.t_iconic,
            "t_visible": self.t_visible,
            "t_above_f": self.t_above_f,
            "t_z_index_enumwindows": self.z_all,
            "t_z_index_shown": self.z_shown,
            "v_percent": self.v_percent,
        })
    }
}

pub fn describe_window(handle: isize) -> Value {
    json!({
        "hwnd": handle,
        "pid": window_pid(handle),
        "class": class_of(handle),
        "rect": rect_of(handle),
        "iconic": is_iconic(handle),
        "visible": is_visible(handle),
    })
}
