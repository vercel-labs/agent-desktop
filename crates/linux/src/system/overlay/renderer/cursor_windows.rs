//! The windows of one cursor, made as its parts first appear and stacked
//! bottom to top in the extension's order.
use std::collections::BTreeMap;
use std::time::Instant;

use agent_desktop_core::AdapterError;

use super::piece::Piece;
use super::request::Owner;
use super::slot::Slot;
use super::slot_window::SlotWindow;
use super::stage::Stage;
use super::text::LabelFont;

pub(crate) struct CursorWindows {
    owner: Vec<u8>,
    slots: BTreeMap<Slot, SlotWindow>,
    raised_for: Option<Instant>,
}

/// The ownership property's value: the session and agent as a JSON pair,
/// the form the stacking reader and the screenshot path parse.
fn owner_value(owner: &Owner) -> Vec<u8> {
    serde_json::to_vec(&(&owner.session, &owner.agent)).unwrap_or_default()
}

/// One frame of a cursor as the windows show it.
pub(crate) struct Frame<'a> {
    pub(crate) pieces: &'a [Piece],
    pub(crate) woke: Instant,
    pub(crate) font: Option<&'a LabelFont>,
    pub(crate) composited: bool,
}

impl CursorWindows {
    pub(crate) fn new(owner: &Owner) -> Self {
        Self {
            owner: owner_value(owner),
            slots: BTreeMap::new(),
            raised_for: None,
        }
    }

    /// Shows `frame`, hiding every part it leaves out. The windows are raised
    /// above the desktop again when the agent uses the cursor, and whenever
    /// a part appears, so the parts keep their order.
    pub(crate) fn show(&mut self, stage: &Stage, frame: &Frame) -> Result<(), AdapterError> {
        let mut appeared = false;
        for slot in Slot::ALL {
            match frame.pieces.iter().find(|piece| piece.slot == slot) {
                Some(piece) => {
                    let window = match self.slots.entry(slot) {
                        std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(SlotWindow::create(stage, &self.owner)?)
                        }
                    };
                    appeared |= window.show(stage, piece, frame.font, frame.composited)?;
                }
                None => {
                    if let Some(window) = self.slots.get_mut(&slot) {
                        window.hide(stage)?;
                    }
                }
            }
        }
        if appeared || self.raised_for != Some(frame.woke) {
            self.raised_for = Some(frame.woke);
            for window in self.slots.values().filter(|window| window.is_mapped()) {
                window.raise(stage)?;
            }
        }
        Ok(())
    }

    pub(crate) fn destroy(self, stage: &Stage) -> Result<(), AdapterError> {
        for (_, window) in self.slots {
            window.destroy(stage)?;
        }
        Ok(())
    }
}
