use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    InteractionPolicy, KeyCombo, ProcessIdentity, StepMechanism,
};
use std::time::Duration;

use crate::system::escape_popup::{CANCEL, cancel_surface};
use crate::tree::AXElement;

#[path = "key_dispatch_menu.rs"]
mod menu;

pub(crate) fn press_for_app_impl(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    press_for_app(process, combo, policy, deadline)
        .map_err(|error| crate::actions::DeliveryTracker::default().annotate(error))
}

fn press_for_app(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    tracing::debug!(pid = process.pid.get(), "system: press key for app");
    let identity = crate::system::process::process_identity::require_core(&process)?;
    let pid = identity.pid();
    let app = crate::tree::element_for_pid(pid);

    if uses_menu_shortcut(combo, policy) {
        match menu::try_menu_bar_shortcut(&app, combo, deadline) {
            Ok(true) => {
                return post_delivery_process_result(&process, semantic_step("AXPress menu item"));
            }
            Ok(false) => {}
            Err(error) if menu::is_bounded_menu_incomplete(&error) => {
                tracing::debug!("bounded menu lookup fell back to exact PID-targeted key delivery");
            }
            Err(error) => return Err(error),
        }
    }
    if let Some(action) = simple_key_action(combo)
        && let Some(step) = deliver_semantic_key(identity, &app, action, deadline)?
    {
        return post_delivery_process_result(&process, step);
    }

    crate::input::keyboard::keyboard_map::key_name_to_code(&combo.key)?;
    if chord_needs_frontmost(combo, policy) {
        let frontmost = crate::system::focus::is_app_frontmost(pid, deadline)?;
        if let Some(refusal) = background_chord_refusal(combo, frontmost) {
            return Err(refusal);
        }
    }
    if policy.allow_focus_steal {
        crate::system::process::process_identity::require_core(&process)?;
        crate::system::focus::verify_app_focused(pid, deadline)?;
    }
    crate::system::process::process_identity::require_core(&process)?;
    require_focused_element(&app, policy, deadline)?;
    crate::system::process::process_identity::require_core(&process)?;
    crate::input::keyboard::synthesize_key(combo, Some(pid), deadline)?;
    post_delivery_process_result(
        &process,
        ActionStep::succeeded("CGEventPostToPid")
            .with_mechanism(StepMechanism::PhysicalSynthetic)
            .with_verified(false),
    )
}

/// A key equivalent such as cmd+s is matched by the active application's menu
/// bar, so a chord posted to a background process is dispatched but never
/// acted on. Headed delivery verifies the frontmost application itself.
fn chord_needs_frontmost(combo: &KeyCombo, policy: InteractionPolicy) -> bool {
    !policy.allow_focus_steal
        && combo
            .modifiers
            .iter()
            .any(|modifier| !matches!(modifier, agent_desktop_core::Modifier::Shift))
}

fn background_chord_refusal(combo: &KeyCombo, frontmost: bool) -> Option<AdapterError> {
    if frontmost {
        return None;
    }
    Some(
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!(
                "The target application is not frontmost, so the {} chord would not reach its menu shortcuts and no key was sent",
                combo.key
            ),
        )
        .with_details(serde_json::json!({
            "check": "process_focus",
            "cause": "background_chord",
            "physical_delivery_started": false,
        }))
        .with_suggestion(format!(
            "Bring the application to the front yourself, or use the menu item through a semantic route. {}",
            agent_desktop_core::headed_mode_suggestion("brings the application to the foreground before sending keys")
        ))
        .with_disposition(DeliverySemantics::not_delivered()),
    )
}

fn semantic_step(label: &'static str) -> ActionStep {
    ActionStep::succeeded(label)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(false)
}

fn post_delivery_process_result(
    process: &ProcessIdentity,
    step: ActionStep,
) -> Result<ActionResult, AdapterError> {
    crate::system::process::process_identity::require_core(process)
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?;
    let verified = step.verified == Some(true);
    let result = ActionResult::delivered_unverified("press_key").with_steps(vec![step]);
    Ok(if verified {
        result.with_verified_delivery()
    } else {
        result
    })
}

/// Where a key without modifiers lands as an accessibility action.
#[derive(Debug, PartialEq, Eq)]
enum SemanticKeyTarget<T> {
    /// The focus holder advertises the action the key stands for.
    FocusHolder(T),
    /// Escape while no focus holder takes it and the application shows a menu
    /// or a popover: the surface the key would close.
    OpenSurface(T),
}

/// The states in the order they are tried: the focus holder, then, for
/// Escape only, the open surface. `Ok(None)` leaves the key its event route,
/// while a failed surface read propagates instead of reading as absent.
fn choose_key_target<T>(
    focus_holder: Option<T>,
    action: &str,
    open_surface: impl FnOnce() -> Result<Option<T>, AdapterError>,
) -> Result<Option<SemanticKeyTarget<T>>, AdapterError> {
    if let Some(holder) = focus_holder {
        return Ok(Some(SemanticKeyTarget::FocusHolder(holder)));
    }
    if action != CANCEL {
        return Ok(None);
    }
    Ok(open_surface()?.map(SemanticKeyTarget::OpenSurface))
}

fn semantic_key_target(
    identity: crate::system::process::process_identity::ProcessIdentity,
    app: &AXElement,
    action: &str,
    deadline: Deadline,
) -> Result<Option<SemanticKeyTarget<AXElement>>, AdapterError> {
    let focus_holder = focused_element(app, deadline)?.filter(|focused| {
        crate::actions::ax_helpers::advertises_action(focused, action, deadline) == Some(true)
    });
    choose_key_target(focus_holder, action, || {
        open_transient_surface(identity, deadline)
    })
}

/// The menu or popover the application shows. A search that reached its node
/// limit finds none, and the key then takes the event route; every other read
/// failure, including a changed process generation, propagates.
fn open_transient_surface(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    match crate::tree::surfaces::transient_surface_for_process(identity, deadline) {
        Ok(surface) => Ok(surface),
        Err(error) if crate::tree::app_menu::is_surface_search_limit(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Performs the action on the target the key reaches. The process generation is
/// revalidated immediately before the action, and an open surface must belong to
/// that process. A closed surface is verified by reading that it is gone.
fn deliver_semantic_key(
    identity: crate::system::process::process_identity::ProcessIdentity,
    app: &AXElement,
    action: &'static str,
    deadline: Deadline,
) -> Result<Option<ActionStep>, AdapterError> {
    let Some(target) = semantic_key_target(identity, app, action, deadline)? else {
        return Ok(None);
    };
    match &target {
        SemanticKeyTarget::FocusHolder(element) => {
            crate::tree::app_menu::require_generation(identity).map_err(not_delivered)?;
            if !crate::actions::ax_helpers::try_ax_action_or_err(element, action, deadline)? {
                return Ok(None);
            }
            Ok(Some(semantic_step(action)))
        }
        SemanticKeyTarget::OpenSurface(element) => cancel_surface(identity, element, deadline),
    }
}

fn not_delivered(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

fn uses_menu_shortcut(combo: &KeyCombo, policy: InteractionPolicy) -> bool {
    policy.allow_focus_steal && !combo.modifiers.is_empty()
}

fn simple_key_action(combo: &KeyCombo) -> Option<&'static str> {
    if !combo.modifiers.is_empty() {
        return None;
    }
    match combo.key.as_str() {
        "return" | "enter" => Some("AXConfirm"),
        "escape" | "esc" => Some(CANCEL),
        _ => None,
    }
}

fn no_focused_element_error(policy: InteractionPolicy) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Application has no verified focused element for keyboard delivery",
    )
    .with_details(serde_json::json!({
        "check": "process_focus",
        "physical_delivery_started": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
    .with_suggestion(if policy.allow_focus_steal {
        "Ensure the target field has keyboard focus before retrying, or run the menu command without keys: find the item with 'snapshot --app <app> --surface menubar' and click its ref.".to_owned()
    } else {
        format!(
            "{} A headless alternative is to run the menu command without keys: find the item with 'snapshot --app <app> --surface menubar' and click its ref.",
            agent_desktop_core::headed_mode_suggestion("lets the app take focus before sending keys")
        )
    })
}

fn require_focused_element(
    app: &AXElement,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<AXElement, AdapterError> {
    let read = || focused_element(app, deadline);
    let found = deadline.poll_until(
        Duration::from_millis(500),
        Duration::from_millis(5),
        read,
        |read| !matches!(read, Ok(None)),
    )?;
    if let Some(focused) = found {
        return Ok(focused);
    }
    deadline.ensure_remaining()?;
    Err(no_focused_element_error(policy))
}

fn focused_element(app: &AXElement, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    element_attribute(app, "AXFocusedUIElement", deadline)
}

fn element_attribute(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let result =
        crate::tree::properties::attributes::copy_element_attr_result(element, attribute, deadline);
    deadline.ensure_remaining()?;
    result.map_err(|error| read_error(attribute, error))
}

fn read_error(attribute: &str, error: i32) -> AdapterError {
    AdapterError::new(
        if error == accessibility_sys::kAXErrorCannotComplete {
            ErrorCode::Timeout
        } else if error == accessibility_sys::kAXErrorAPIDisabled {
            ErrorCode::PermDenied
        } else if error == accessibility_sys::kAXErrorInvalidUIElement {
            ErrorCode::StaleRef
        } else {
            ErrorCode::ActionFailed
        },
        format!("Accessibility read failed for {attribute} during key dispatch"),
    )
    .with_details(serde_json::json!({ "attribute": attribute, "ax_error": error }))
}

#[cfg(test)]
#[path = "key_dispatch_tests.rs"]
mod tests;
