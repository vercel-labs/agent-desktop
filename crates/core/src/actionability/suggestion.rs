use super::evidence::ActionabilityEvidence;
use super::report::ActionabilityReport;
use crate::actions::action_request::ActionRequest;

/// The text every unmatched actionability failure falls back to.
pub(super) const WAITING_SUGGESTION: &str = "Waiting will not help: this element cannot satisfy the action as targeted. Target an element that advertises the action (check available_actions in a fresh snapshot). For a dropdown without select support, open it with click, inspect its choices, and use explicit keyboard input.";

/// The generic recovery text for a report that ends `ACTION_FAILED`.
pub(super) const ACTION_FAILED_SUGGESTION: &str = "Wait for the target to become actionable, refresh the snapshot, or use an explicit physical/focus command if intended.";

const OFFSCREEN_SUGGESTION: &str = "The target is off screen. Run 'scroll-to' on its ref first, take a fresh snapshot, then retry the action.";

const ITEM_CLICK_PREFIX: &str = "To choose one item, click the item itself: find it by name in a fresh snapshot and click its ref.";

pub(super) fn text_write(policy: crate::InteractionPolicy) -> String {
    if policy.is_headed() {
        "This field has no semantic text-write action. Use 'type' in the current headed mode for physical keyboard input; select the existing text first if you intend to replace it. If typing refuses because the field cannot be focused, use clipboard-set and the app's paste shortcut, then verify the field.".to_owned()
    } else {
        format!(
            "This field has no semantic text-write action. {} For 'type', select the existing text first if you intend to replace it.",
            crate::headed_mode_suggestion(
                "sends synthetic keys to the field and may take the foreground"
            )
        )
    }
}

/// Refines the suggestion `finish` picked for a failed report. An
/// `ACTION_FAILED` report whose only blocker is the live offscreen visibility
/// refusal gets the scroll-to path instead of a wait; a `select` that missed a
/// collection of choosable items gets the click-the-item path prepended. Every
/// other suggestion passes through unchanged.
pub(super) fn refine(
    suggestion: String,
    report: &ActionabilityReport,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
) -> String {
    if suggestion == ACTION_FAILED_SUGGESTION
        && evidence.state.offscreen == Some(true)
        && report.only_blocking_check_is_offscreen()
    {
        return format!(
            "{OFFSCREEN_SUGGESTION} If semantic scrolling is unavailable, {}",
            crate::headed_mode_suggestion("moves the pointer for physical scrolling")
        );
    }
    if suggestion == WAITING_SUGGESTION
        && matches!(request.action, crate::Action::Select(_))
        && crate::tree::roles::is_item_collection_role(&evidence.state.role)
    {
        format!("{ITEM_CLICK_PREFIX} {suggestion}")
    } else {
        suggestion
    }
}
