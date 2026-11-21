//! Arrangements, the trial bracket and the settle loop.
//!
//! `F` is a window the harness owns and keeps in the foreground. `T` owns the
//! scrollable element. A trial reads every signal, runs one call, then keeps
//! reading until the signals hold still for 500 ms (2 s cap).

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use uiautomation::patterns::UIScrollPattern;
use uiautomation::{UIAutomation, UIElement};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetSystemMetrics, SetForegroundWindow, SetWindowPos, ShowWindow,
};

use crate::calls::Outcome;
use crate::hook;
use crate::state::{self, PercentReader, Watch};

const SETTLE: Duration = Duration::from_millis(500);
const SETTLE_CAP: Duration = Duration::from_millis(2000);
const POLL: Duration = Duration::from_millis(20);
const PREPARE_SETTLE: Duration = Duration::from_millis(400);
const PREPARE_CAP: Duration = Duration::from_millis(4000);
const SW_SHOWNOACTIVATE: i32 = 4;
const SW_SHOWMINNOACTIVE: i32 = 7;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_NOOWNERZORDER: u32 = 0x0200;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

#[derive(Clone, Copy, PartialEq)]
pub enum Arrangement {
    Beside,
    Covered,
    Minimized,
}

impl Arrangement {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "beside" => Some(Self::Beside),
            "covered" => Some(Self::Covered),
            "minimized" => Some(Self::Minimized),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Beside => "beside",
            Self::Covered => "covered",
            Self::Minimized => "minimized",
        }
    }
}

pub struct Stage<'a> {
    pub automation: &'a UIAutomation,
    pub f: isize,
    pub t: isize,
    pub f_focus: Option<UIElement>,
    pub scroll: Option<UIScrollPattern>,
    pub arrangement: Arrangement,
}

fn place(handle: isize, rect: [i32; 4]) {
    unsafe {
        SetWindowPos(
            state::raw(handle),
            std::ptr::null_mut(),
            rect[0],
            rect[1],
            rect[2],
            rect[3],
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
    };
}

fn layout(arrangement: Arrangement) -> ([i32; 4], [i32; 4]) {
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let target_width = 720.min(width / 2 - 20);
    let target_height = 520.min(height - 140);
    match arrangement {
        Arrangement::Covered => (
            [0, 0, (target_width + 200).min(width), (target_height + 200).min(height - 50)],
            [80, 80, target_width, target_height],
        ),
        _ => (
            [0, 0, width - target_width - 40, height - 80],
            [width - target_width - 20, 40, target_width, target_height],
        ),
    }
}

/// Sizes `T` as the visible arrangements do, without activating it, so the
/// element lookup sees the layout the trials run against.
pub fn place_for_discovery(target: isize) {
    if state::is_iconic(target) || !state::is_visible(target) {
        unsafe { ShowWindow(state::raw(target), SW_SHOWNOACTIVATE) };
    }
    place(target, layout(Arrangement::Beside).1);
    std::thread::sleep(Duration::from_millis(1500));
}

fn contains(outer: [i32; 4], inner: [i32; 4]) -> bool {
    outer[0] <= inner[0] && outer[1] <= inner[1] && outer[2] >= inner[2] && outer[3] >= inner[3]
}

fn disjoint(a: [i32; 4], b: [i32; 4]) -> bool {
    a[2] <= b[0] || b[2] <= a[0] || a[3] <= b[1] || b[3] <= a[1]
}

impl Stage<'_> {
    fn watch(&self) -> Watch<'_> {
        Watch {
            automation: self.automation,
            f: self.f,
            t: self.t,
            scroll: self.scroll.as_ref(),
        }
    }

    /// Puts both windows where the arrangement wants them, without activating `T`.
    pub fn enter(&mut self, arrangement: Arrangement) -> Value {
        self.arrangement = arrangement;
        let (f_rect, t_rect) = layout(arrangement);
        if state::is_iconic(self.f) {
            unsafe { ShowWindow(state::raw(self.f), SW_SHOWNOACTIVATE) };
        }
        place(self.f, f_rect);
        if arrangement != Arrangement::Minimized {
            if state::is_iconic(self.t) || !state::is_visible(self.t) {
                unsafe { ShowWindow(state::raw(self.t), SW_SHOWNOACTIVATE) };
            }
            place(self.t, t_rect);
        }
        json!({ "arrangement": arrangement.name(), "f_rect_requested": f_rect, "t_rect_requested": t_rect })
    }

    fn activate_f(&self, actions: &mut Vec<&'static str>) {
        if state::foreground() == self.f {
            return;
        }
        if let Some(element) = self.f_focus.as_ref() {
            let _ = element.set_focus();
            actions.push("f_activated_by_uia_set_focus");
            if wait_for(Duration::from_millis(1200), || state::foreground() == self.f) {
                return;
            }
        }
        let foreground_thread = state::window_thread(state::foreground());
        let own_thread = unsafe { GetCurrentThreadId() };
        unsafe {
            AttachThreadInput(own_thread, foreground_thread, 1);
            BringWindowToTop(state::raw(self.f));
            SetForegroundWindow(state::raw(self.f));
            AttachThreadInput(own_thread, foreground_thread, 0);
        }
        actions.push("f_activated_by_attach_thread_input");
        wait_for(Duration::from_millis(1200), || state::foreground() == self.f);
    }

    /// Restores the invariants a trial starts from and reports whether they hold.
    fn prepare(&self) -> Value {
        let mut actions: Vec<&'static str> = Vec::new();
        for _ in 0..3 {
            match self.arrangement {
                Arrangement::Minimized => {
                    if !state::is_iconic(self.t) {
                        unsafe { ShowWindow(state::raw(self.t), SW_SHOWMINNOACTIVE) };
                        actions.push("t_minimized_without_activation");
                    }
                }
                _ => {
                    if state::is_iconic(self.t) || !state::is_visible(self.t) {
                        unsafe { ShowWindow(state::raw(self.t), SW_SHOWNOACTIVATE) };
                        actions.push("t_restored_without_activation");
                    }
                }
            }
            if state::is_iconic(self.f) {
                unsafe { ShowWindow(state::raw(self.f), SW_SHOWNOACTIVATE) };
                actions.push("f_restored");
            }
            self.activate_f(&mut actions);
            self.wait_quiet();
            if self.invariants().0 {
                break;
            }
        }
        let (holds, checks) = self.invariants();
        json!({ "holds": holds, "actions": actions, "checks": checks })
    }

    fn wait_quiet(&self) {
        let watch = Watch {
            scroll: None,
            ..self.watch()
        };
        let mut percent = PercentReader::new();
        let started = Instant::now();
        let mut last = state::read(&watch, &mut percent);
        let mut last_change = Instant::now();
        while started.elapsed() < PREPARE_CAP && last_change.elapsed() < PREPARE_SETTLE {
            std::thread::sleep(POLL);
            let now = state::read(&watch, &mut percent);
            if !last.settled_with(&now) {
                last = now;
                last_change = Instant::now();
            }
        }
    }

    fn invariants(&self) -> (bool, Value) {
        let f_rect = state::rect_of(self.f).unwrap_or([0; 4]);
        let t_rect = state::rect_of(self.t).unwrap_or([0; 4]);
        let f_foreground = state::foreground() == self.f;
        let windows = state::top_level_windows();
        let rank = |handle: isize| windows.iter().position(|other| *other == handle);
        let f_above_t = matches!((rank(self.f), rank(self.t)), (Some(f), Some(t)) if f < t);
        let placed = match self.arrangement {
            Arrangement::Beside => {
                !state::is_iconic(self.t) && state::is_visible(self.t) && disjoint(f_rect, t_rect)
            }
            Arrangement::Covered => {
                !state::is_iconic(self.t)
                    && state::is_visible(self.t)
                    && contains(f_rect, t_rect)
                    && f_above_t
            }
            Arrangement::Minimized => state::is_iconic(self.t),
        };
        (
            f_foreground && placed,
            json!({
                "f_is_foreground": f_foreground,
                "t_placed_as_arranged": placed,
                "f_above_t": f_above_t,
                "f_rect": f_rect,
                "t_rect": t_rect,
                "t_iconic": state::is_iconic(self.t),
            }),
        )
    }

    /// One bracketed trial around `act`.
    pub fn trial(&self, call: &str, index: usize, act: &mut dyn FnMut() -> Outcome) -> Value {
        let prepared = self.prepare();
        let watch = self.watch();
        let mut percent = PercentReader::new();
        let before = state::read(&watch, &mut percent);
        let start_tick = unsafe { GetTickCount() };
        let started = Instant::now();
        let outcome = act();
        let returned = Instant::now();
        let mut timeline: Vec<Value> = Vec::new();
        let mut ever: Vec<&'static str> = Vec::new();
        let mut raised_past = 0usize;
        let mut last = before.clone();
        let mut last_change = returned;
        let mut capped = false;
        loop {
            let now = state::read(&watch, &mut percent);
            let fields = last.changed_fields(&now);
            if !fields.is_empty() {
                raised_past += last.raised_past(&now);
                for field in &fields {
                    if !ever.contains(field) {
                        ever.push(*field);
                    }
                }
                timeline.push(json!({
                    "t_ms": started.elapsed().as_millis() as u64,
                    "fields": fields,
                    "state": now.to_json(),
                }));
                last = now;
                last_change = Instant::now();
            }
            if last_change.elapsed() >= SETTLE {
                break;
            }
            if returned.elapsed() >= SETTLE_CAP {
                capped = true;
                break;
            }
            std::thread::sleep(POLL);
        }
        let events = hook::since(start_tick);
        let last_fields = before.changed_fields(&last);
        json!({
            "arrangement": self.arrangement.name(),
            "call": call,
            "trial": index,
            "prepared": prepared,
            "outcome": outcome.to_json(),
            "before": before.to_json(),
            "after": last.to_json(),
            "changed_at_any_time": ever,
            "changed_at_end": last_fields,
            "raised_past_shown_windows": raised_past,
            "timeline": timeline,
            "settle": {
                "observed_ms": started.elapsed().as_millis() as u64,
                "capped_at_2s": capped,
                "percent_reads_slow": percent.slow_reads,
                "percent_reads_failed": percent.failed_reads,
            },
            "winevents": hook::summarize(&events, start_tick),
            "hook_delivered_in_process": hook::delivered(start_tick),
        })
    }

    /// Leaves `T` restored and not active, so the harness can close it.
    pub fn leave(&self) {
        if state::is_iconic(self.t) {
            unsafe { ShowWindow(state::raw(self.t), SW_SHOWNOACTIVATE) };
        }
    }
}

fn wait_for(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        if done() {
            return true;
        }
        std::thread::sleep(POLL);
    }
    done()
}
