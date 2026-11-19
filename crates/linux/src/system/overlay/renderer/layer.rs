//! Every agent cursor the renderer draws, one per session and agent id, and
//! the requests that change them, played as the GNOME Shell extension's
//! cursor layer plays them.
use std::time::{Duration, Instant};

use agent_desktop_core::Point;

use super::cursor::Cursor;
use super::desk::Desk;
use super::request::{LocateRequest, Owner, Request, Styled};
use super::travel::Ticket;
use super::travel_order::TravelOrder;
use crate::system::overlay::cursor_start::CursorStart;

/// How long one capture may keep the cursors hidden. A capture whose process
/// never asks for them back cannot hide them for longer.
pub(crate) const CONCEAL_LIMIT: Duration = Duration::from_secs(30);
const UNANCHORED_OFFSET: Point = Point {
    x: -180.0,
    y: 108.0,
};

#[derive(Debug, Default)]
pub(crate) struct CursorLayer {
    cursors: Vec<(Owner, Cursor)>,
    conceal_depth: u32,
    conceal_until: Option<Instant>,
}

fn reaches(entry: &Owner, owner: &Owner, every_agent: bool) -> bool {
    entry.session == owner.session && (every_agent || entry.agent == owner.agent)
}

impl CursorLayer {
    pub(crate) fn is_empty(&self) -> bool {
        self.cursors.is_empty()
    }

    pub(crate) fn cursors(&self) -> impl Iterator<Item = &(Owner, Cursor)> {
        self.cursors.iter()
    }

    /// The sessions that have a cursor.
    pub(crate) fn sessions(&self) -> Vec<String> {
        let mut sessions: Vec<String> = self
            .cursors
            .iter()
            .map(|(owner, _)| owner.session.clone())
            .collect();
        sessions.sort();
        sessions.dedup();
        sessions
    }

    fn find(&self, owner: &Owner) -> Option<&Cursor> {
        self.cursors
            .iter()
            .find(|(entry, _)| entry == owner)
            .map(|(_, cursor)| cursor)
    }

    fn owned(&mut self, styled: &Styled, now: Instant) -> &mut Cursor {
        let index = match self
            .cursors
            .iter()
            .position(|(entry, _)| *entry == styled.owner)
        {
            Some(index) => index,
            None => {
                let cursor = Cursor::new(styled.look.clone(), now, self.conceal_depth > 0);
                self.cursors.push((styled.owner.clone(), cursor));
                self.cursors.len() - 1
            }
        };
        &mut self.cursors[index].1
    }

    /// Where a travel to the request's destination sets off: the cursor's
    /// tip, or for a cursor with no known position, a point below and left
    /// of the destination on the same monitor, the way the macOS overlay
    /// starts.
    pub(crate) fn locate(&self, request: &LocateRequest, desk: &dyn Desk) -> CursorStart {
        let cursor = self.find(&request.owner);
        let moves = cursor.map_or(0, |cursor| cursor.motion().moves);
        if let Some(tip) = cursor
            .filter(|cursor| cursor.motion().anchored)
            .and_then(|cursor| cursor.motion().tip.as_ref())
        {
            return CursorStart {
                x: tip.x,
                y: tip.y,
                moves,
            };
        }
        let screen = desk.monitor_at(&request.destination);
        let clamp = |value: f64, low: f64, size: f64| value.max(low).min(low + size);
        CursorStart {
            x: clamp(
                request.destination.x + UNANCHORED_OFFSET.x,
                screen.x,
                screen.width,
            ),
            y: clamp(
                request.destination.y + UNANCHORED_OFFSET.y,
                screen.y,
                screen.height,
            ),
            moves,
        }
    }

    /// Plays one request. Gives back the connections it has answered, which
    /// include its own unless it is a travel still on its way.
    pub(crate) fn update(
        &mut self,
        request: Request,
        ticket: Ticket,
        now: Instant,
        desk: &dyn Desk,
    ) -> Vec<Ticket> {
        let mut answered = vec![ticket];
        match request {
            Request::Conceal => self.conceal(now),
            Request::Reveal => self.reveal(),
            Request::Disable(session) => answered.extend(self.end_session(&session, now)),
            Request::Hide(owner) => self.each(&owner, |cursor| cursor.hide()),
            Request::Show(owner) => self.each(&owner, |cursor| cursor.show(now)),
            Request::Enable(styled) => self.owned(&styled, now).enable(&styled, desk, now),
            Request::Travel {
                styled,
                path,
                pointer,
                clear_caret,
                drag,
            } => {
                let order = TravelOrder {
                    path,
                    pointer,
                    clear_caret,
                    drag,
                    ticket,
                };
                return self.owned(&styled, now).travel(&styled, order, now);
            }
            Request::Effect {
                styled,
                point,
                target,
                path,
            } => {
                answered.extend(self.owned(&styled, now).effect(
                    &styled,
                    (point, target),
                    path,
                    now,
                ));
            }
            Request::Caret {
                styled,
                rect,
                follow,
            } => {
                if !follow || self.find(&styled.owner).is_some() {
                    self.owned(&styled, now)
                        .caret_to(&styled, rect, follow, now);
                }
            }
        }
        answered
    }

    fn each(&mut self, owner: &Owner, mut change: impl FnMut(&mut Cursor)) {
        let every_agent = owner.agent.is_none();
        for (_, cursor) in self
            .cursors
            .iter_mut()
            .filter(|(entry, _)| reaches(entry, owner, every_agent))
        {
            change(cursor);
        }
    }

    /// Removes every cursor of `session`, answering travels they were on.
    pub(crate) fn end_session(&mut self, session: &str, now: Instant) -> Vec<Ticket> {
        let mut answered = Vec::new();
        self.cursors.retain_mut(|(owner, cursor)| {
            if owner.session != session {
                return true;
            }
            answered.extend(cursor.finish_travel(now));
            false
        });
        answered
    }

    /// Moves every cursor along to `now`, and gives back the travels that
    /// arrived. A conceal older than the limit is lifted.
    pub(crate) fn tick(&mut self, now: Instant) -> Vec<Ticket> {
        if self.conceal_until.is_some_and(|until| now >= until) {
            self.conceal_until = None;
            self.conceal_depth = 0;
            self.set_concealed(false);
        }
        self.cursors
            .iter_mut()
            .filter_map(|(_, cursor)| cursor.tick(now))
            .collect()
    }

    /// Hides every cursor for one capture. Captures can overlap, so each
    /// conceal is counted and the cursors come back only when every capture
    /// has revealed, or when the newest conceal is older than the limit.
    fn conceal(&mut self, now: Instant) {
        self.conceal_depth += 1;
        self.conceal_until = Some(now + CONCEAL_LIMIT);
        self.set_concealed(true);
    }

    fn reveal(&mut self) {
        if self.conceal_depth == 0 {
            return;
        }
        self.conceal_depth -= 1;
        if self.conceal_depth > 0 {
            return;
        }
        self.conceal_until = None;
        self.set_concealed(false);
    }

    fn set_concealed(&mut self, concealed: bool) {
        for (_, cursor) in &mut self.cursors {
            cursor.set_concealed(concealed);
        }
    }

    /// The soonest moment the drawing changes without a request: the next
    /// frame while anything moves, fades or plays, otherwise when a shown
    /// cursor starts to rest or a conceal runs out.
    pub(crate) fn next_change(&self, now: Instant, frame: Duration) -> Option<Instant> {
        let mut soonest = self.conceal_until;
        for (_, cursor) in &self.cursors {
            if let Some(at) = cursor.next_change(now, frame) {
                soonest = Some(soonest.map_or(at, |soonest| soonest.min(at)));
            }
        }
        soonest
    }
}

#[cfg(test)]
#[path = "layer_tests.rs"]
mod tests;
