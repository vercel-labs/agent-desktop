//! Draws the cursor layer on the X server: one set of windows per cursor,
//! made when a cursor appears and destroyed when it goes.
use std::time::{Duration, Instant};

use agent_desktop_core::AdapterError;
use x11rb::connection::Connection;

use super::cursor_windows::{CursorWindows, Frame};
use super::desk::Desk;
use super::layer::CursorLayer;
use super::request::Owner;
use super::scene::{Scenery, compose};
use super::stage::Stage;
use super::text::LabelFont;
use crate::system::overlay::cursor_overlay::error;

/// How often the renderer looks again for a compositing manager, which can
/// start or stop while cursors are shown.
const COMPOSITOR_CHECK: Duration = Duration::from_secs(1);

pub(crate) struct Painter {
    windows: Vec<(Owner, CursorWindows)>,
    font: Option<LabelFont>,
    composited: Option<(bool, Instant)>,
}

impl Painter {
    pub(crate) fn new(font: Option<LabelFont>) -> Self {
        Self {
            windows: Vec::new(),
            font,
            composited: None,
        }
    }

    fn composited(&mut self, stage: &Stage, now: Instant) -> Result<bool, AdapterError> {
        match self.composited {
            Some((composited, checked))
                if now.saturating_duration_since(checked) < COMPOSITOR_CHECK =>
            {
                Ok(composited)
            }
            _ => {
                let composited = stage.is_composited()?;
                self.composited = Some((composited, now));
                Ok(composited)
            }
        }
    }

    /// Draws every cursor of `layer` as it is at `now`, and removes the
    /// windows of cursors that are gone.
    pub(crate) fn paint(
        &mut self,
        stage: &Stage,
        layer: &CursorLayer,
        desk: &dyn Desk,
        now: Instant,
    ) -> Result<(), AdapterError> {
        let composited = self.composited(stage, now)?;
        let mut kept = Vec::with_capacity(self.windows.len());
        for (owner, windows) in self.windows.drain(..) {
            if layer.cursors().any(|(live, _)| *live == owner) {
                kept.push((owner, windows));
            } else {
                windows.destroy(stage)?;
            }
        }
        self.windows = kept;
        let scenery = Scenery {
            now,
            desk,
            font: self.font.as_ref(),
        };
        for (owner, cursor) in layer.cursors() {
            let index = match self.windows.iter().position(|(drawn, _)| drawn == owner) {
                Some(index) => index,
                None => {
                    self.windows
                        .push((owner.clone(), CursorWindows::new(owner)));
                    self.windows.len() - 1
                }
            };
            let pieces = compose(cursor, &scenery);
            let frame = Frame {
                pieces: &pieces,
                woke: cursor.presence().woke,
                font: scenery.font,
                composited,
            };
            self.windows[index].1.show(stage, &frame)?;
        }
        stage.connection.flush().map_err(error)
    }
}
