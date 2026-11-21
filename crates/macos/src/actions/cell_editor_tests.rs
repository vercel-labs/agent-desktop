use super::{CellEdit, CellTarget, Replacement, cell_target, replacement, write_through};
use crate::actions::cell_read_back::ReadBack;
use crate::actions::cell_write_failure::{left_open, nothing_typed};
use agent_desktop_core::{AdapterError, DeliveryOutcome, DeliverySemantics, ErrorCode};

#[test]
fn an_empty_editor_is_typed_into_without_a_selection() {
    assert_eq!(replacement("", "42"), Replacement::Type);
}

#[test]
fn held_text_is_selected_before_the_new_text_is_typed() {
    assert_eq!(replacement("41", "42"), Replacement::SelectThenType(2));
}

#[test]
fn held_text_is_selected_and_deleted_for_an_empty_value() {
    assert_eq!(replacement("41", ""), Replacement::SelectThenDelete(2));
}

#[test]
fn an_editor_that_already_holds_the_value_gets_nothing() {
    assert_eq!(replacement("42", "42"), Replacement::AlreadyHeld);
    assert_eq!(replacement("", ""), Replacement::AlreadyHeld);
}

#[test]
fn the_selection_length_counts_utf16_units() {
    assert_eq!(replacement("é😀", "x"), Replacement::SelectThenType(3));
}

#[test]
fn a_cell_that_was_pressed_and_gave_no_editor_fails_as_delivered() {
    assert_eq!(nothing_typed(false).unwrap(), DeliveryOutcome::NotDelivered);

    let error = nothing_typed(true).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn text_left_in_an_editor_the_application_moved_fails_as_delivered() {
    assert_eq!(left_open(false).unwrap(), DeliveryOutcome::NotDelivered);

    let error = left_open(true).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_cell_that_takes_a_value_of_its_own_is_not_pressed_for_an_editor() {
    let never_asked = || panic!("a cell that takes its own value is not judged for a press");

    assert_eq!(cell_target(Some(true), never_asked), CellTarget::Neither);
    assert_eq!(cell_target(None, never_asked), CellTarget::Neither);
    assert_eq!(cell_target(Some(false), || false), CellTarget::Cell);
    assert_eq!(
        cell_target(Some(false), || true),
        CellTarget::ForegroundCell
    );
}

fn unverified() -> DeliveryOutcome {
    DeliveryOutcome::from_delivery(true, false)
}

fn verified() -> DeliveryOutcome {
    DeliveryOutcome::from_delivery(true, true)
}

/// A cell whose editor is open from the start, or opens on the press numbered
/// `opens_on_press`, and that records every call made on it.
struct ScriptedCell {
    open: bool,
    opens_on_press: usize,
    press: DeliveryOutcome,
    typed: Result<DeliveryOutcome, ErrorCode>,
    read_back: ReadBack,
    keeps_focus: bool,
    presses: usize,
    calls: Vec<&'static str>,
}

fn scripted() -> ScriptedCell {
    ScriptedCell {
        open: false,
        opens_on_press: 1,
        press: unverified(),
        typed: Ok(unverified()),
        read_back: ReadBack::Matches,
        keeps_focus: true,
        presses: 0,
        calls: Vec::new(),
    }
}

impl CellEdit for ScriptedCell {
    type Editor = ();

    fn editor(&mut self) -> Result<Option<()>, AdapterError> {
        self.calls.push("editor");
        Ok(self.open.then_some(()))
    }

    fn press(&mut self) -> Result<DeliveryOutcome, AdapterError> {
        self.calls.push("press");
        self.presses += 1;
        Ok(self.press)
    }

    fn opened_editor(&mut self) -> Result<Option<()>, AdapterError> {
        self.calls.push("opened_editor");
        Ok((self.presses == self.opens_on_press).then_some(()))
    }

    fn replace(&mut self, _: &(), _: &str) -> Result<DeliveryOutcome, AdapterError> {
        self.calls.push("replace");
        self.typed
            .clone()
            .map_err(|code| AdapterError::new(code, "the editor refused the text"))
    }

    fn holds_focus(&mut self, _: &()) -> Result<bool, AdapterError> {
        self.calls.push("holds_focus");
        Ok(self.keeps_focus)
    }

    fn commit(&mut self, _: &()) -> Result<(), AdapterError> {
        self.calls.push("commit");
        Ok(())
    }

    fn read_back(&mut self, _: &str) -> Result<ReadBack, AdapterError> {
        self.calls.push("read_back");
        Ok(self.read_back)
    }
}

#[test]
fn an_open_editor_is_replaced_and_committed_without_a_press() {
    let mut cell = ScriptedCell {
        open: true,
        ..scripted()
    };

    assert_eq!(write_through(&mut cell, "42").unwrap(), verified());
    assert_eq!(
        cell.calls,
        ["editor", "replace", "holds_focus", "commit", "read_back"]
    );
}

#[test]
fn a_cell_that_stays_empty_after_the_commit_fails_as_delivered() {
    let mut cell = ScriptedCell {
        open: true,
        read_back: ReadBack::Empty,
        ..scripted()
    };

    let error = write_through(&mut cell, "42").unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(
        cell.calls,
        ["editor", "replace", "holds_focus", "commit", "read_back"]
    );
}

#[test]
fn a_cell_that_shows_another_value_stays_unverified() {
    let mut cell = ScriptedCell {
        read_back: ReadBack::Differs,
        ..scripted()
    };

    assert_eq!(write_through(&mut cell, "42").unwrap(), unverified());
    assert_eq!(cell.calls.last(), Some(&"read_back"));
}

#[test]
fn an_unreadable_cell_stays_unverified_and_never_fails() {
    let mut cell = ScriptedCell {
        read_back: ReadBack::Unknown,
        ..scripted()
    };

    assert_eq!(write_through(&mut cell, "42").unwrap(), unverified());
    assert_eq!(cell.calls.last(), Some(&"read_back"));
}

#[test]
fn a_press_that_only_selects_the_cell_is_followed_by_one_more() {
    let mut cell = ScriptedCell {
        opens_on_press: 2,
        ..scripted()
    };

    assert_eq!(write_through(&mut cell, "42").unwrap(), verified());
    assert_eq!(
        cell.calls,
        [
            "editor",
            "press",
            "opened_editor",
            "press",
            "opened_editor",
            "replace",
            "holds_focus",
            "commit",
            "read_back"
        ]
    );
}

#[test]
fn a_cell_that_gives_no_editor_after_two_presses_gets_no_text() {
    let mut cell = ScriptedCell {
        opens_on_press: 3,
        ..scripted()
    };

    let error = write_through(&mut cell, "42").unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(cell.presses, 2);
    assert!(!cell.calls.contains(&"replace"));
}

#[test]
fn a_press_the_cell_does_not_take_hands_the_write_to_the_next_rung() {
    let mut cell = ScriptedCell {
        press: DeliveryOutcome::NotDelivered,
        ..scripted()
    };

    assert_eq!(
        write_through(&mut cell, "42").unwrap(),
        DeliveryOutcome::NotDelivered
    );
    assert_eq!(cell.calls, ["editor", "press"]);
}

#[test]
fn an_edit_the_application_moved_out_of_the_editor_is_not_committed() {
    let mut cell = ScriptedCell {
        keeps_focus: false,
        ..scripted()
    };

    let error = write_through(&mut cell, "=A1").unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(!cell.calls.contains(&"commit"));
}

#[test]
fn a_failure_after_the_press_is_reported_as_delivered() {
    let mut cell = ScriptedCell {
        typed: Err(ErrorCode::AppUnresponsive),
        ..scripted()
    };

    let error = write_through(&mut cell, "42").unwrap_err();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(!cell.calls.contains(&"commit"));
}
