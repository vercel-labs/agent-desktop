//! What a native action does to the user's foreground session. The classifier
//! names the reason an action takes the foreground, and `require` refuses an
//! effect the interaction policy does not permit before the application is
//! touched, so headless delivery never shows the user a menu or a dialog.
//! Writing `AXFocused` moves focus only inside the target application
//! (M-W0-14), so it has no effect here and headless `focus` is delivered.
//!
//! The reasons are `shows_menu` and `opens_dialog`. A menu overlay is drawn
//! over the user's window, takes their focused element, and AppKit orders the
//! owning window front; no AX path selects a popup item or opens a context
//! menu without it. A dialog is predicted by an owner-judgment rule: a label
//! that ends in an ellipsis ("…" U+2026 or "..."), after trailing whitespace
//! is trimmed, is the convention shared by GTK, Qt and macOS applications for
//! a control that opens a dialog. It is a predictor, not a proof: no
//! accessibility property says that a control opens a window. A label that
//! cannot be read counts as one.

use agent_desktop_core::{Action, AdapterError, Deadline, Effect, InteractionPolicy};

use crate::tree::AXElement;

const MENU_OWNER_ROLES: &[&str] = &["AXPopUpButton", "AXMenuButton", "AXComboBox"];
const ELLIPSES: &[&str] = &["\u{2026}", "..."];
const SHOWS_MENU: &str = "shows_menu";
const OPENS_DIALOG: &str = "opens_dialog";

/// Refuses `operation` with `POLICY_DENIED` when `policy` does not permit
/// `effect`, and reports `kind` as the reason the action takes the foreground.
pub(crate) fn require(
    policy: InteractionPolicy,
    effect: Effect,
    operation: &str,
    kind: &'static str,
) -> Result<(), AdapterError> {
    policy.require(effect, operation).map_err(|error| {
        error.with_details(serde_json::json!({ "foreground_effect": true, "kind": kind }))
    })
}

/// The foreground effect of an element action: a menu or a dialog takes the
/// user's focus. Only a press-type action reads the target, and only when the
/// policy denies focus, because no other policy refuses it.
pub(crate) fn foreground_effect(
    element: &AXElement,
    request: &agent_desktop_core::ActionRequest,
    deadline: Deadline,
) -> Option<agent_desktop_core::DeclaredEffect> {
    if request.policy.allow_focus_steal {
        return None;
    }
    let kind = action_effect(&request.action, || read_target(element, deadline))?;
    Some(agent_desktop_core::DeclaredEffect {
        effect: Effect::FocusSteal,
        kind,
    })
}

fn action_effect(
    action: &Action,
    read: impl FnOnce() -> Option<PressTarget>,
) -> Option<&'static str> {
    match action {
        Action::RightClick => Some(SHOWS_MENU),
        Action::Click | Action::Expand | Action::Toggle | Action::Check | Action::Uncheck => {
            press_effect(action, &read())
        }
        _ => None,
    }
}

/// A press on a descendant the caller did not target must not show a menu or
/// open a dialog under any policy, so the click fallback skips such a child.
/// A child whose role and label cannot be read is skipped as well.
pub(crate) fn descendant_press_disturbs(element: &AXElement, deadline: Deadline) -> bool {
    press_effect(&Action::Click, &read_target(element, deadline)).is_some()
}

/// `None` when the role or a label could not be read.
#[derive(Debug, Default)]
struct PressTarget {
    role: Option<String>,
    labels: Vec<String>,
}

fn press_effect(action: &Action, target: &Option<PressTarget>) -> Option<&'static str> {
    let Some(target) = target else {
        return Some(OPENS_DIALOG);
    };
    let menu_owner = target
        .role
        .as_deref()
        .is_some_and(|role| MENU_OWNER_ROLES.contains(&role));
    if menu_owner && matches!(action, Action::Click | Action::Expand) {
        return Some(SHOWS_MENU);
    }
    let opens_dialog = target.labels.iter().any(|label| {
        let label = label.trim_end();
        ELLIPSES.iter().any(|ellipsis| label.ends_with(ellipsis))
    });
    (!matches!(action, Action::Expand) && opens_dialog).then_some(OPENS_DIALOG)
}

fn read_target(element: &AXElement, deadline: Deadline) -> Option<PressTarget> {
    use crate::tree::properties::node_attribute_status::{DESCRIPTION, LABEL, TITLE};
    let requirements = agent_desktop_core::EvidenceRequirements {
        role: true,
        name: true,
        ..Default::default()
    };
    let mut usage = crate::tree::observation_usage::ObservationUsage::with_defaults();
    let read = crate::tree::properties::element::fetch_node_attrs_with_status_for(
        element,
        requirements,
        deadline,
        &mut usage,
    );
    let status = read.status;
    if status.role_unknown()
        || [TITLE, LABEL, DESCRIPTION]
            .iter()
            .any(|i| status.field_unknown(*i))
    {
        return None;
    }
    let names = read.attrs.name_evidence;
    Some(PressTarget {
        role: read.attrs.role,
        labels: [names.native_title, names.explicit_label, names.description]
            .into_iter()
            .flatten()
            .collect(),
    })
}

#[cfg(test)]
#[path = "background_effect_tests.rs"]
mod tests;
