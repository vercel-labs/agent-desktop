use std::time::Duration;

use agent_desktop_core::{
    Action, ActionResult, AdapterError, ChainRung, DeliveryOutcome, DeliverySemantics, ErrorCode,
};
use serde_json::json;

use super::click::{POLL_STEP, poll_until};
use super::context::Target;
use super::write::mutation::{WriteKind, WriteReply};
use super::write::write_call::{Call, invoke};
use crate::actions::chain::run_semantic_chain;
use crate::tree::COMPONENT_INTERFACE;
use crate::tree::vocabulary::state_bits::FOCUSED;

const FOCUS_SETTLE: Duration = Duration::from_millis(500);
const GRAB_FOCUS_LABEL: &str = "Component.GrabFocus";
const GRAB_FOCUS_REJECTED: &str = "grab_focus_rejected";
const FOCUS_WINDOW_HINT: &str = "Use focus-window to focus the owning window. This application does not let accessibility clients move focus to an element inside it";

pub(super) fn corroborated(first: Option<bool>, second: Option<bool>) -> Option<bool> {
    match first {
        Some(true) => second,
        other => other,
    }
}

fn grab_rejected() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The application rejected Component.GrabFocus, so keyboard focus did not move to the element",
    )
    .with_suggestion(FOCUS_WINDOW_HINT)
    .with_details(json!({ "cause": GRAB_FOCUS_REJECTED }))
    .with_disposition(DeliverySemantics::not_delivered())
}

pub(super) fn judge_focus(
    focused_before: Option<bool>,
    grab: impl FnOnce() -> Result<WriteReply, AdapterError>,
    observe: impl FnOnce() -> Option<bool>,
) -> Result<DeliveryOutcome, AdapterError> {
    if focused_before == Some(true) {
        return Ok(DeliveryOutcome::Satisfied);
    }
    match grab()? {
        WriteReply::Absent => Err(grab_rejected()),
        WriteReply::Done => Ok(DeliveryOutcome::observed(observe())),
    }
}

fn grab_focus(target: &Target<'_>) -> Result<WriteReply, AdapterError> {
    invoke(
        target,
        Call {
            interface: COMPONENT_INTERFACE,
            method: "GrabFocus",
            body: &(),
            kind: WriteKind::GrabFocus,
        },
    )
}

fn focus_read_back(target: &Target<'_>) -> Option<bool> {
    let read = || target.view().ok().and_then(|after| after.state(FOCUSED));
    let first = poll_until(target.deadline, FOCUS_SETTLE, POLL_STEP, read, |focused| {
        *focused == Some(true)
    });
    if first == Some(true) {
        std::thread::sleep(POLL_STEP);
    }
    corroborated(first, read())
}

pub(crate) fn focus(target: &Target<'_>) -> Result<ActionResult, AdapterError> {
    let focused_before = target.view()?.state(FOCUSED);
    let run = || {
        judge_focus(
            focused_before,
            || grab_focus(target),
            || focus_read_back(target),
        )
    };
    let mut rungs = [ChainRung {
        label: GRAB_FOCUS_LABEL,
        effects: &[],
        run: Box::new(run),
    }];
    run_semantic_chain(
        &Action::SetFocus,
        &mut rungs,
        target.deadline,
        FOCUS_WINDOW_HINT,
    )
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
