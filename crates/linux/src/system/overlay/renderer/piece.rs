//! One window's worth of a cursor at one instant.
use super::art::Art;
use super::slot::Slot;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Piece {
    pub(crate) slot: Slot,
    /// The window's top-left corner on the screen.
    pub(crate) origin: (i32, i32),
    pub(crate) art: Art,
    pub(crate) opacity: f64,
}
