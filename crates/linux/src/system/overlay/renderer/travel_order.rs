//! A travel request as a cursor plays it.
use super::request::Frames;
use super::travel::Ticket;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TravelOrder {
    pub(crate) path: Frames,
    /// Whether the pointer image shows on arrival.
    pub(crate) pointer: bool,
    /// Whether the text caret clears as the travel sets off.
    pub(crate) clear_caret: bool,
    /// Whether the travel goes to where a drag begins.
    pub(crate) drag: bool,
    pub(crate) ticket: Ticket,
}
