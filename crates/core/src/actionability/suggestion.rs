use std::borrow::Cow;

use super::evidence::ActionabilityEvidence;
use super::report::ActionabilityReport;
use crate::actions::action_request::ActionRequest;

/// The text every unmatched actionability failure falls back to.
pub(super) const WAITING_SUGGESTION: &str = "Waiting will not help: this element cannot satisfy the action as targeted. Target an element that advertises the action (check available_actions in a fresh snapshot). For a dropdown without select support, open it with click, inspect its choices, and use explicit keyboard input.";

/// The generic recovery text for a report that ends `ACTION_FAILED`.
pub(super) const ACTION_FAILED_SUGGESTION: &str = "Wait for the target to become actionable, refresh the snapshot, or use an explicit physical/focus command if intended.";

const OFFSCREEN_SUGGESTION: &str = "The target is off screen. Run 'scroll-to' on its ref first (some lists can be scrolled only with --headed), take a fresh snapshot, then retry the action.";

const ITEM_CLICK_PREFIX: &str = "To choose one item, click the item itself: find it by name in a fresh snapshot and click its ref.";

/// Refines the suggestion `finish` picked for a failed report. An
/// `ACTION_FAILED` report whose only blocker is the live offscreen visibility
/// refusal gets the scroll-to path instead of a wait; a `select` that missed a
/// collection of choosable items gets the click-the-item path prepended. Every
/// other suggestion passes through unchanged.
pub(super) fn refine(
    suggestion: &'static str,
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
        Cow::Borrowed(suggestion)
    }
}
