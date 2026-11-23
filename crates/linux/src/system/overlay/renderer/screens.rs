//! The monitors and work area the renderer places cursors on, read from the
//! X server and kept for a short while so a frame costs no round trip.
use std::time::{Duration, Instant};

use agent_desktop_core::{AdapterError, Point, Rect};
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

use super::desk::Desk;
use super::stage::Stage;
use crate::system::overlay::cursor_overlay::error;

const REREAD_AFTER: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Screens {
    /// Each monitor and whether it is the primary one.
    monitors: Vec<(Rect, bool)>,
    work_area: Option<Rect>,
    read_at: Option<Instant>,
}

fn intersection(a: &Rect, b: &Rect) -> Option<Rect> {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    (right > left && bottom > top).then_some(Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

fn contains(rect: &Rect, at: &Point) -> bool {
    at.x >= rect.x && at.x < rect.x + rect.width && at.y >= rect.y && at.y < rect.y + rect.height
}

impl Screens {
    pub(crate) fn new(monitors: Vec<(Rect, bool)>, work_area: Option<Rect>) -> Self {
        Self {
            monitors,
            work_area,
            read_at: None,
        }
    }

    /// Reads the monitors and work area again when the last reading is old.
    pub(crate) fn refresh(&mut self, stage: &Stage, now: Instant) -> Result<(), AdapterError> {
        if self
            .read_at
            .is_some_and(|read_at| now.saturating_duration_since(read_at) < REREAD_AFTER)
        {
            return Ok(());
        }
        *self = Self {
            read_at: Some(now),
            ..read(stage)?
        };
        Ok(())
    }

    /// The monitor that holds `at`, the primary one otherwise, and the root
    /// window's size when RandR names none.
    pub(crate) fn monitor_at(&self, at: &Point, fallback: &Rect) -> Rect {
        self.monitors
            .iter()
            .find(|(monitor, _)| contains(monitor, at))
            .or_else(|| self.monitors.iter().find(|(_, primary)| *primary))
            .or_else(|| self.monitors.first())
            .map_or_else(|| *fallback, |(monitor, _)| *monitor)
    }

    /// That monitor less the panels and docks the window manager reserves,
    /// or the whole monitor when it reserves none there.
    pub(crate) fn work_area_at(&self, at: &Point, fallback: &Rect) -> Rect {
        let monitor = self.monitor_at(at, fallback);
        self.work_area
            .as_ref()
            .and_then(|area| intersection(area, &monitor))
            .unwrap_or(monitor)
    }
}

fn read(stage: &Stage) -> Result<Screens, AdapterError> {
    let connection = &stage.connection;
    let monitors = connection
        .randr_get_monitors(stage.root, true)
        .map_err(error)?
        .reply()
        .map(|reply| {
            reply
                .monitors
                .into_iter()
                .map(|monitor| {
                    let bounds = Rect {
                        x: f64::from(monitor.x),
                        y: f64::from(monitor.y),
                        width: f64::from(monitor.width),
                        height: f64::from(monitor.height),
                    };
                    (bounds, monitor.primary)
                })
                .collect()
        })
        .unwrap_or_default();
    let workarea = connection
        .intern_atom(false, b"_NET_WORKAREA")
        .map_err(error)?
        .reply()
        .map_err(error)?
        .atom;
    let work_area = connection
        .get_property(false, stage.root, workarea, AtomEnum::CARDINAL, 0, 4)
        .map_err(error)?
        .reply()
        .ok()
        .and_then(|reply| reply.value32().map(Iterator::collect::<Vec<u32>>))
        .and_then(|values| match values.as_slice() {
            [x, y, width, height] => Some(Rect {
                x: f64::from(*x),
                y: f64::from(*y),
                width: f64::from(*width),
                height: f64::from(*height),
            }),
            _ => None,
        });
    Ok(Screens::new(monitors, work_area))
}

/// The display as the cursors see it during one frame.
pub(crate) struct X11Desk<'a> {
    pub(crate) stage: &'a Stage,
    pub(crate) screens: &'a Screens,
    pub(crate) root_size: Rect,
}

impl Desk for X11Desk<'_> {
    fn pointer(&self) -> Point {
        let reply = self
            .stage
            .connection
            .query_pointer(self.stage.root)
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        reply.map_or(Point { x: 0.0, y: 0.0 }, |reply| Point {
            x: f64::from(reply.root_x),
            y: f64::from(reply.root_y),
        })
    }

    fn monitor_at(&self, at: &Point) -> Rect {
        self.screens.monitor_at(at, &self.root_size)
    }

    fn work_area_at(&self, at: &Point) -> Rect {
        self.screens.work_area_at(at, &self.root_size)
    }
}

#[cfg(test)]
#[path = "screens_tests.rs"]
mod tests;
