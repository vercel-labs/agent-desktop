use std::borrow::Cow;

use super::evidence::ActionabilityEvidence;
use super::report::ActionabilityReport;
use crate::actions::{action_request::ActionRequest, capability};
use crate::{Action, ErrorCode, adapter::PlatformAdapter};

/// The text every unmatched actionability failure falls back to.
pub(super) const WAITING_SUGGESTION: &str = "Waiting will not help: this element cannot satisfy the action as targeted. Target an element that advertises the action (check available_actions in a fresh snapshot). For a dropdown without select support, open it with click, inspect its choices, and use explicit keyboard input.";

/// The generic recovery text for a report that ends `ACTION_FAILED`.
pub(super) const ACTION_FAILED_SUGGESTION: &str = "Wait for the target to become actionable, refresh the snapshot, or use an explicit physical/focus command if intended.";

const OFFSCREEN_SUGGESTION: &str = "The target is off screen. Run 'scroll-to' on its ref first (some lists can be scrolled only with --headed), take a fresh snapshot, then retry the action.";

const ITEM_CLICK_PREFIX: &str = "To choose one item, click the item itself: find it by name in a fresh snapshot and click its ref.";

const CELL_SUGGESTION: &str = "This cell is not directly editable. Activate it, take a fresh snapshot, and target the text editor if one appears.";

const SET_FOCUS_SUGGESTION: &str = "Choose an element advertising SetFocus in a fresh snapshot. To focus the owning window instead, use focus-window.";

const SET_VALUE_FOR_TYPE_SUGGESTION: &str = "This element accepts a semantic text write. Use 'set-value' on its ref with the text; it replaces the whole value. 'type' needs physical keyboard focus, which headless mode does not allow.";

const SELECT_FOR_POINTER_SUGGESTION: &str = "This element advertises Select but not Click, so a click needs physical input that headless mode does not allow. Use 'select' on its ref, or on the list that holds it.";

const NO_POINTER_SESSION_SUGGESTION: &str = "This element has no semantic path for the action, and this session cannot send physical pointer input, so headed mode cannot help. Use an action the element advertises (see available_actions in a fresh snapshot), or report the limitation to the user.";

const POLICY_SUGGESTION: &str = "This element has no semantic path for the action, and headless mode does not allow the physical input it needs. Use an action the element advertises (see available_actions in a fresh snapshot). Use --headed only if the user asked for physical input; otherwise report the limitation to the user.";

const TEXT_WRITE_HEADED: &str = "This field has no semantic text-write action. Use 'type' in the current headed mode for physical keyboard input; select the existing text first if you intend to replace it. If typing refuses because the field cannot be focused, use clipboard-set and the app's paste shortcut, then verify the field.";

const TEXT_WRITE_HEADLESS: &str = "This field has no semantic text-write action. Use '--headed type' for physical keyboard input; select the existing text first if you intend to replace it. If typing refuses because the field cannot be focused, use clipboard-set and the app's paste shortcut, then verify the field.";

/// Picks the step that can fix this refusal. Each branch names a step the
/// target and the session can take; a refusal no branch recognises falls to
/// the waiting text.
pub(super) fn select(
    code: &ErrorCode,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    report: &ActionabilityReport,
    adapter: Option<&dyn PlatformAdapter>,
) -> Cow<'static, str> {
    let role = evidence.state.role.as_str();
    let actions = &evidence.available_actions;
    let editable_failed = report.checks.iter().any(|check| {
        check.check == "editable" && check.status == super::status::ActionabilityStatus::Fail
    });
    let text_field = matches!(role, "textfield" | "combobox");
    Cow::Borrowed(match code {
        ErrorCode::ActionFailed => ACTION_FAILED_SUGGESTION,
        _ if role == "cell" && editable_failed => CELL_SUGGESTION,
        ErrorCode::ActionNotSupported if matches!(request.action, Action::SetFocus) => {
            SET_FOCUS_SUGGESTION
        }
        ErrorCode::ActionNotSupported
            if text_field && matches!(request.action, Action::SetValue(_) | Action::Clear) =>
        {
            if request.policy.is_headed() {
                TEXT_WRITE_HEADED
            } else {
                TEXT_WRITE_HEADLESS
            }
        }
        ErrorCode::PolicyDenied
            if matches!(request.action, Action::TypeText(_))
                && capability::contains(actions, capability::SET_VALUE) =>
        {
            SET_VALUE_FOR_TYPE_SUGGESTION
        }
        ErrorCode::PolicyDenied
            if request.action.requires_hit_test()
                && capability::contains(actions, capability::SELECT)
                && !capability::contains(actions, capability::CLICK) =>
        {
            SELECT_FOR_POINTER_SUGGESTION
        }
        ErrorCode::PolicyDenied
            if request.action.requires_hit_test()
                && adapter.is_some_and(|adapter| adapter.physical_pointer_refusal().is_some()) =>
        {
            NO_POINTER_SESSION_SUGGESTION
        }
        ErrorCode::PolicyDenied => POLICY_SUGGESTION,
        _ => WAITING_SUGGESTION,
    })
}

/// Refines the suggestion `finish` picked for a failed report. An
/// `ACTION_FAILED` report whose only blocker is the live offscreen visibility
/// refusal gets the scroll-to path instead of a wait; a `select` that missed a
/// collection of choosable items gets the click-the-item path prepended. Every
/// other suggestion passes through unchanged.
pub(super) fn refine(
    suggestion: Cow<'static, str>,
    report: &ActionabilityReport,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
) -> Cow<'static, str> {
    if suggestion == ACTION_FAILED_SUGGESTION
        && evidence.state.offscreen == Some(true)
        && report.only_blocking_check_is_offscreen()
    {
        return Cow::Borrowed(OFFSCREEN_SUGGESTION);
    }
    if suggestion == WAITING_SUGGESTION
        && matches!(request.action, crate::Action::Select(_))
        && crate::tree::roles::is_item_collection_role(&evidence.state.role)
    {
        Cow::Owned(format!("{ITEM_CLICK_PREFIX} {suggestion}"))
    } else {
        suggestion
    }
}
