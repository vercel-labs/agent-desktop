//! The failures a value write through a cell's editor can end with.

use agent_desktop_core::{AdapterError, DeliveryOutcome, DeliverySemantics, ErrorCode};

const LEFT_OPEN_SUGGESTION: &str = "Commit the open edit with a key press scoped to the application, such as press return --app <app>, then read the cell to verify.";
const NO_EDITOR_SUGGESTION: &str = "Take a fresh snapshot before another write: the press can select the cell or open another surface.";
const CELL_TOOK_NOTHING_SUGGESTION: &str = "Bring the application to the front and retry, or type into the cell with --headed only if the user explicitly asked for that, then read the cell to verify the write.";

/// The end of a write whose editor lost the keyboard focus before the commit,
/// as it does when the application moves the edit into an editor of its own.
/// Return is not sent to another element. Text that was sent stays on record
/// as a failure, so the caller does not read it as a committed write.
pub(crate) fn left_open(sent: bool) -> Result<DeliveryOutcome, AdapterError> {
    if !sent {
        return Ok(DeliveryOutcome::NotDelivered);
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "The text was typed, and the application moved the edit out of the cell's editor before it was committed",
    )
    .with_suggestion(LEFT_OPEN_SUGGESTION)
    .with_disposition(DeliverySemantics::delivered_unverified()))
}

/// The end of a write that typed nothing. A press that was sent stays on
/// record as a failure, so the caller does not read it as a write.
pub(crate) fn nothing_typed(pressed: bool) -> Result<DeliveryOutcome, AdapterError> {
    if !pressed {
        return Ok(DeliveryOutcome::NotDelivered);
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "The cell was pressed and nothing was typed: it gave no text editor, or the editor's text could not be read or selected",
    )
    .with_suggestion(NO_EDITOR_SUGGESTION)
    .with_disposition(DeliverySemantics::delivered_unverified()))
}

/// The end of a write whose cell still shows nothing after the commit: the
/// application took the keys and dropped them instead of the text.
pub(crate) fn cell_took_nothing() -> Result<DeliveryOutcome, AdapterError> {
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "The text was typed and committed, but the cell still shows nothing",
    )
    .with_suggestion(CELL_TOOK_NOTHING_SUGGESTION)
    .with_disposition(DeliverySemantics::delivered_unverified()))
}
