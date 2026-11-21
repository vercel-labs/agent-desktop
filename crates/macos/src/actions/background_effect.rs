//! What a native action does to the user's foreground session, and the
//! interaction policy each effect needs. An action whose effect the policy
//! does not permit is refused before the application is touched, so headless
//! delivery never shows the user a menu or a dialog. Writing `AXFocused` moves
//! focus only inside the target application (M-W0-14), so it has no effect
//! here and headless `focus` is delivered.

use agent_desktop_core::{Action, AdapterError, Deadline, InteractionPolicy};

use crate::tree::AXElement;

const MENU_OWNER_ROLES: &[&str] = &["AXPopUpButton", "AXMenuButton", "AXComboBox"];
const ELLIPSES: &[&str] = &["\u{2026}", "..."];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    /// A menu overlay is drawn over the user's window, takes their focused
    /// element, and AppKit orders the owning window front. No AX path selects
    /// a popup item or opens a context menu without it.
    ShowsMenu,
    /// Owner-judgment rule: a label that ends in an ellipsis ("…" U+2026 or
    /// "..."), after trailing whitespace is trimmed, is the convention shared
    /// by GTK, Qt and macOS applications for a control that opens a dialog. It
    /// is a predictor, not a proof: no accessibility property says that a
    /// control opens a window. A label that cannot be read counts as one.
    OpensDialog,
    /// Synthetic keys are typed into the frontmost application's key focus.
    TakesKeyFocus,
    /// Synthetic pointer or keyboard events reach the frontmost application.
    PhysicalInput,
}

impl Effect {
    pub(crate) fn permitted(self, policy: InteractionPolicy) -> bool {
        match self {
            Self::PhysicalInput => policy.is_headed(),
            Self::ShowsMenu | Self::OpensDialog | Self::TakesKeyFocus => policy.allow_focus_steal,
        }
    }

    pub(crate) fn refuse_unless_permitted(
        self,
        policy: InteractionPolicy,
    ) -> Result<(), AdapterError> {
        if self.permitted(policy) {
            return Ok(());
        }
        Err(
            AdapterError::policy_denied_for_policy(self.refusal(), policy).with_details(
                serde_json::json!({ "foreground_effect": true, "effect": self.name() }),
            ),
        )
    }

    fn name(self) -> &'static str {
        match self {
            Self::ShowsMenu => "shows_menu",
            Self::OpensDialog => "opens_dialog",
            Self::TakesKeyFocus => "takes_key_focus",
            Self::PhysicalInput => "physical_input",
        }
    }

    fn refusal(self) -> &'static str {
        match self {
            Self::ShowsMenu => {
                "Headless delivery refused: this action shows a menu over the user's window and takes their focused element"
            }
            Self::OpensDialog => {
                "Headless delivery refused: the element's label ends in an ellipsis, so it opens a dialog or sheet that can activate the application"
            }
            Self::TakesKeyFocus => {
                "Headless delivery refused: physical keyboard input needs focus permission"
            }
            Self::PhysicalInput => {
                "Headless delivery refused: physical pointer or keyboard input needs the headed policy"
            }
        }
    }
}

/// Refuses an element action whose effect the policy does not permit. Only a
/// press-type action reads the target, and only when the policy denies focus.
pub(crate) fn refuse_disturbing_action(
    element: &AXElement,
    action: &Action,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if policy.allow_focus_steal {
        return Ok(());
    }
    action_effect(action, || read_target(element, deadline))
        .map_or(Ok(()), |effect| effect.refuse_unless_permitted(policy))
}

fn action_effect(action: &Action, read: impl FnOnce() -> Option<PressTarget>) -> Option<Effect> {
    match action {
        Action::RightClick => Some(Effect::ShowsMenu),
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

fn press_effect(action: &Action, target: &Option<PressTarget>) -> Option<Effect> {
    let Some(target) = target else {
        return Some(Effect::OpensDialog);
    };
    let menu_owner = target
        .role
        .as_deref()
        .is_some_and(|role| MENU_OWNER_ROLES.contains(&role));
    if menu_owner && matches!(action, Action::Click | Action::Expand) {
        return Some(Effect::ShowsMenu);
    }
    let opens_dialog = target.labels.iter().any(|label| {
        let label = label.trim_end();
        ELLIPSES.iter().any(|ellipsis| label.ends_with(ellipsis))
    });
    (!matches!(action, Action::Expand) && opens_dialog).then_some(Effect::OpensDialog)
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
