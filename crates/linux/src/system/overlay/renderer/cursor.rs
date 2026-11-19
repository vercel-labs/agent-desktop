//! One agent's cursor, kept the way the GNOME Shell extension keeps it. Time
//! is passed in, so what the cursor shows is a function of the instant it is
//! drawn at, and a travel arrives once that instant passes its end.
use std::time::{Duration, Instant};

use agent_desktop_core::{Point, Rect};

use super::cursor_face::Faces;
use super::cursor_label::CursorLabel;
use super::desk::Desk;
use super::effect::Effect;
use super::motion::Motion;
use super::presence::{IDLE_REST, Presence};
use super::request::{Frames, Look, Styled};
use super::travel::{Ticket, Travel};
use super::travel_order::TravelOrder;

#[derive(Debug, Clone)]
pub(crate) struct Cursor {
    look: Look,
    faces: Faces,
    motion: Motion,
    label: CursorLabel,
    effect: Option<Effect>,
    caret: Option<Rect>,
    presence: Presence,
}

impl Cursor {
    pub(crate) fn new(look: Look, now: Instant, concealed: bool) -> Self {
        let mut faces = Faces::default();
        faces.restyle(&look);
        Self {
            look,
            faces,
            motion: Motion::default(),
            label: CursorLabel::default(),
            effect: None,
            caret: None,
            presence: Presence {
                concealed,
                ..Presence::new(now)
            },
        }
    }

    pub(crate) fn look(&self) -> &Look {
        &self.look
    }

    pub(crate) fn faces(&self) -> &Faces {
        &self.faces
    }

    pub(crate) fn motion(&self) -> &Motion {
        &self.motion
    }

    pub(crate) fn label(&self) -> &CursorLabel {
        &self.label
    }

    pub(crate) fn playing_effect(&self) -> Option<&Effect> {
        self.effect.as_ref()
    }

    pub(crate) fn caret(&self) -> Option<&Rect> {
        self.caret.as_ref()
    }

    pub(crate) fn presence(&self) -> &Presence {
        &self.presence
    }

    fn restyle(&mut self, styled: &Styled) {
        self.look = styled.look.clone();
        self.faces.restyle(&self.look);
    }

    fn relabel(&mut self, styled: &Styled) {
        self.restyle(styled);
        self.label.set(styled.label.clone());
    }

    /// Marks the cursor used at `now`. A caret the finished rest cleared
    /// stays cleared.
    fn wake(&mut self, now: Instant) {
        if self.presence.has_rested(now) {
            self.caret = None;
        }
        self.presence.woke = now;
    }

    pub(crate) fn set_concealed(&mut self, concealed: bool) {
        self.presence.concealed = concealed;
    }

    pub(crate) fn enable(&mut self, styled: &Styled, desk: &dyn Desk, now: Instant) {
        self.wake(now);
        self.relabel(styled);
        self.motion.moves = 0;
        self.motion.dragging = false;
        if self.motion.tip.is_none() {
            self.motion.tip = Some(desk.pointer());
        }
        self.motion.anchored = true;
        self.presence.hidden = false;
        self.label.reveal(now);
    }

    /// Ends the travel under way, as though it had arrived, and gives back
    /// the connection that waited on it.
    pub(crate) fn finish_travel(&mut self, now: Instant) -> Option<Ticket> {
        let travel = self.motion.travel.take()?;
        self.motion.pointing = travel.pointer;
        if let Some(destination) = travel.destination() {
            self.motion.tip = Some(destination);
        }
        self.label.reveal(now);
        Some(travel.ticket)
    }

    /// Starts a travel. The arrow shows on the way, and the label and the
    /// pointer image wait for the arrival. Answers the travel it replaced,
    /// and this one too when it has no length.
    pub(crate) fn travel(
        &mut self,
        styled: &Styled,
        order: TravelOrder,
        now: Instant,
    ) -> Vec<Ticket> {
        let mut answered: Vec<Ticket> = self.finish_travel(now).into_iter().collect();
        self.wake(now);
        self.relabel(styled);
        if order.clear_caret {
            self.caret = None;
        }
        self.motion.pointing = false;
        self.motion.moves += 1;
        self.motion.anchored = true;
        self.motion.dragging = order.drag;
        let travel = Travel {
            path: order.path,
            started: now,
            pointer: order.pointer,
            ticket: order.ticket,
        };
        self.motion.tip = travel.at(now).or(self.motion.tip.take());
        self.motion.travel = Some(travel);
        answered.extend(self.tick(now));
        answered
    }

    /// Moves a travelling cursor along to `now`, and gives back the
    /// connection of a travel that arrived.
    pub(crate) fn tick(&mut self, now: Instant) -> Option<Ticket> {
        let travel = self.motion.travel.as_ref()?;
        if travel.has_arrived(now) {
            return self.finish_travel(now);
        }
        self.motion.tip = travel.at(now).or(self.motion.tip.take());
        None
    }

    /// Shows the click ripple and outline at `point`. Nothing waits on it.
    pub(crate) fn effect(
        &mut self,
        styled: &Styled,
        (point, target): (Point, Option<Rect>),
        path: Frames,
        now: Instant,
    ) -> Option<Ticket> {
        let answered = self.finish_travel(now);
        self.effect = None;
        self.wake(now);
        self.relabel(styled);
        self.motion.dragging = false;
        self.motion.tip = Some(point);
        self.label.reveal(now);
        if !path.frames.is_empty() {
            self.effect = Some(Effect {
                path,
                started: now,
                target,
            });
        }
        answered
    }

    /// Places the agent's text caret, or with `follow` moves only a caret
    /// that is already showing. A follow while the cursor rests leaves it
    /// resting.
    pub(crate) fn caret_to(
        &mut self,
        styled: &Styled,
        rect: Option<Rect>,
        follow: bool,
        now: Instant,
    ) {
        if follow && self.presence.is_resting(now) {
            return;
        }
        self.wake(now);
        self.restyle(styled);
        if follow && self.caret.is_none() {
            return;
        }
        match rect {
            Some(rect) => self.caret = Some(rect),
            None if follow => self.caret = None,
            None => {}
        }
    }

    /// The soonest moment this cursor's drawing changes without a request.
    pub(crate) fn next_change(&self, now: Instant, frame: Duration) -> Option<Instant> {
        let playing = self.motion.travel.is_some()
            || self
                .effect
                .as_ref()
                .is_some_and(|effect| effect.at(now).is_some())
            || self.label.is_easing(now);
        if playing {
            return Some(now + frame);
        }
        if !self.presence.is_shown() || self.presence.has_rested(now) {
            return None;
        }
        if self.presence.is_resting(now) {
            return Some(now + frame);
        }
        Some(self.presence.woke + IDLE_REST)
    }

    /// Dispatch hides the cursors around every headed command. A hide that
    /// cancels a drag takes the cursor off screen and drops its anchor, so the
    /// next travel does not set off from the stale grab point. Any other hide
    /// leaves it where it landed and the next travel glides on from there, as
    /// on macOS.
    pub(crate) fn hide(&mut self) {
        if !std::mem::take(&mut self.motion.dragging) {
            return;
        }
        self.presence.hidden = true;
        self.motion.anchored = false;
        self.caret = None;
    }

    pub(crate) fn show(&mut self, now: Instant) {
        self.wake(now);
        self.presence.hidden = false;
        self.label.reveal(now);
    }
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
