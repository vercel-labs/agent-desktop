//! Out-of-context WinEvent hook for foreground, focus and minimize events.
//!
//! The hook thread owns a message pump for the whole run. Each event keeps the
//! tick the system stamped on it, so a trial takes the events stamped at or
//! after its call started, whatever the delivery lag was.

use std::sync::Mutex;
use std::sync::mpsc;
use std::thread::JoinHandle;

use serde_json::{Value, json};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostThreadMessageW,
};

use crate::state::{class_of, window_pid};

pub const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
pub const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
pub const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
pub const EVENT_OBJECT_FOCUS: u32 = 0x8005;
const WINEVENT_OUTOFCONTEXT: u32 = 0;
const WM_QUIT: u32 = 0x0012;

#[derive(Clone)]
pub struct HookEvent {
    pub event: u32,
    pub hwnd: isize,
    pub pid: u32,
    pub class: String,
    pub id_object: i32,
    pub id_child: i32,
    pub tick: u32,
}

static EVENTS: Mutex<Vec<HookEvent>> = Mutex::new(Vec::new());

unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    tick: u32,
) {
    let handle = hwnd as isize;
    if let Ok(mut events) = EVENTS.lock() {
        events.push(HookEvent {
            event,
            hwnd: handle,
            pid: window_pid(handle),
            class: class_of(handle),
            id_object,
            id_child,
            tick,
        });
    }
}

pub struct Hook {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl Hook {
    pub fn start() -> Result<Hook, String> {
        let (ready, started) = mpsc::channel();
        let join = std::thread::spawn(move || unsafe {
            let ranges = [
                (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
                (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
                (EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS),
            ];
            let hooks: Vec<HWINEVENTHOOK> = ranges
                .iter()
                .map(|(low, high)| {
                    SetWinEventHook(
                        *low,
                        *high,
                        std::ptr::null_mut(),
                        Some(on_event),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT,
                    )
                })
                .collect();
            let installed = hooks.iter().all(|hook| !hook.is_null());
            let _ = ready.send((GetCurrentThreadId(), installed));
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                DispatchMessageW(&message);
            }
            for hook in hooks {
                if !hook.is_null() {
                    UnhookWinEvent(hook);
                }
            }
        });
        let (thread_id, installed) = started
            .recv()
            .map_err(|error| format!("hook thread did not start: {error}"))?;
        if !installed {
            return Err("SetWinEventHook failed for at least one event range".into());
        }
        Ok(Hook {
            thread_id,
            join: Some(join),
        })
    }

    pub fn stop(mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Events the system stamped at or after `tick`.
pub fn since(tick: u32) -> Vec<HookEvent> {
    EVENTS
        .lock()
        .map(|events| {
            events
                .iter()
                .filter(|event| event.tick.wrapping_sub(tick) < 0x8000_0000)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// How many events the hook delivered in this process so far, and the tick of
/// the newest one relative to `tick`: shows a hook that is installed but silent.
pub fn delivered(tick: u32) -> Value {
    EVENTS
        .lock()
        .map(|events| {
            json!({
                "total": events.len(),
                "newest_ms_after_call_start": events.last().map(|event| event.tick.wrapping_sub(tick) as i32),
            })
        })
        .unwrap_or(Value::Null)
}

pub fn event_name(event: u32) -> &'static str {
    match event {
        EVENT_SYSTEM_FOREGROUND => "EVENT_SYSTEM_FOREGROUND",
        EVENT_SYSTEM_MINIMIZESTART => "EVENT_SYSTEM_MINIMIZESTART",
        EVENT_SYSTEM_MINIMIZEEND => "EVENT_SYSTEM_MINIMIZEEND",
        EVENT_OBJECT_FOCUS => "EVENT_OBJECT_FOCUS",
        _ => "OTHER",
    }
}

/// Counts per event plus one row per event, relative to the call start tick.
pub fn summarize(events: &[HookEvent], start_tick: u32) -> Value {
    let count = |kind: u32| events.iter().filter(|event| event.event == kind).count();
    let rows: Vec<Value> = events
        .iter()
        .map(|event| {
            json!({
                "event": event_name(event.event),
                "hwnd": event.hwnd,
                "pid": event.pid,
                "class": event.class,
                "id_object": event.id_object,
                "id_child": event.id_child,
                "t_ms": event.tick.wrapping_sub(start_tick),
            })
        })
        .collect();
    json!({
        "foreground": count(EVENT_SYSTEM_FOREGROUND),
        "object_focus": count(EVENT_OBJECT_FOCUS),
        "minimize_start": count(EVENT_SYSTEM_MINIMIZESTART),
        "minimize_end": count(EVENT_SYSTEM_MINIMIZEEND),
        "rows": rows,
    })
}
