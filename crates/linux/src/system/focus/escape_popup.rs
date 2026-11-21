//! Escape for an application that does not own the active window.
//!
//! The key route sends the key only to the active window, so a key for a
//! background application is refused. A native popup that the application
//! opened stays open in the background on some toolkits, and the key cannot
//! reach it. Web documents are excluded, and a candidate must show an open
//! popup menu child and advertise a dismissal verb (or GTK 3 combo box `Press`,
//! L4-4) so Escape never activates a button's primary handler.
//!
//! Three states, in this order: the key route is allowed and nothing here
//! runs; the key route is refused for focus and a dismissible native popup
//! owner exists in the target window; otherwise the refusal stays as it was.
//! The owner's reply is the answer, so a rejected action delivers nothing and
//! an owner that does not answer leaves the delivery uncertain; the key
//! route's refusal replaces neither.

use std::time::Duration;

use agent_desktop_core::{
    Action, ActionResult, AdapterError, ChainRung, Deadline, DeliveryOutcome, KeyCombo,
    StepMechanism, WindowInfo,
};

use crate::actions::action_call::{do_action, resolve_verb};
use crate::actions::chain::run_semantic_chain;
use crate::actions::context::Target;
use crate::actions::write::mutation::WriteReply;
use crate::system::window::window_id;
use crate::system::window::window_ops::windows_of_process;
use crate::system::{REGISTRY_ROOT_PATH, SharedBus, within};
use crate::tree::menu::observe_menu::{rooted, scan_walk_structure};
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind, read_toolkit};
use crate::tree::vocabulary::role_numbers::{
    COMBO_BOX, MENU, POPUP_MENU, PUSH_BUTTON, PUSH_BUTTON_MENU, TOGGLE_BUTTON, is_web_document,
};
use crate::tree::vocabulary::state_bits::{HAS_POPUP, SHOWING, StateBits};
use crate::tree::{AtspiElement, ObservedRead, WalkedNode, get_live_actions};

const SCAN_NODES: usize = 600;
const OWNER_NODES: usize = 64;
const POPUP_CONTAINER_CHILDREN: usize = 16;
const CLOSE_SETTLE: Duration = Duration::from_millis(1000);
const CLOSE_STEP: Duration = Duration::from_millis(50);
const FOCUS_CHECKS: [&str; 2] = ["window_active", "exact_window_focus"];
const SUGGESTION: &str = "Refresh the snapshot and check whether the popup is still open.";
const CLOSE_LABEL: &str = "Action.DoAction";
const DISMISS_VERBS: [&str; 3] = ["collapse", "cancel", "close"];
const GTK3_COMBO_DISMISS_VERBS: [&str; 4] = ["collapse", "cancel", "close", "press"];

pub(super) fn is_plain_escape(combo: &KeyCombo) -> bool {
    combo.modifiers.is_empty() && combo.key.eq_ignore_ascii_case("escape")
}

pub(super) fn is_focus_refusal(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("check"))
        .and_then(|check| check.as_str())
        .is_some_and(|check| FOCUS_CHECKS.contains(&check))
}

fn bits(node: &WalkedNode<ObservedRead>) -> Option<StateBits<'_>> {
    node.payload
        .node
        .traits
        .states
        .present()
        .map(|words| StateBits::new(words))
}

fn role(node: &WalkedNode<ObservedRead>) -> Option<u32> {
    node.payload.node.traits.role.present().copied()
}

fn showing_popup(node: &WalkedNode<ObservedRead>) -> bool {
    !node.in_web_document
        && !is_web_document(role(node))
        && matches!(role(node), Some(MENU | POPUP_MENU))
        && bits(node).is_some_and(|bits| bits.has(SHOWING))
}

/// Whether a native node owns a popup that is visibly open now: a combo box,
/// or a button with `HAS_POPUP`, that has a showing `MENU` or `POPUP_MENU`
/// child. Web documents are excluded, and `EXPANDED` alone never qualifies an
/// owner. A tree row, an expander or a table cell is never an owner: Escape
/// does not collapse a tree.
pub(super) fn is_open_owner(node: &WalkedNode<ObservedRead>) -> bool {
    if node.in_web_document || is_web_document(role(node)) {
        return false;
    }
    let Some(bits) = bits(node) else {
        return false;
    };
    let owner = match role(node) {
        Some(COMBO_BOX) => true,
        Some(PUSH_BUTTON | PUSH_BUTTON_MENU | TOGGLE_BUTTON) => bits.has(HAS_POPUP),
        _ => false,
    };
    owner && node.children.iter().any(showing_popup)
}

/// The dismissal verbs Escape may invoke on an open popup owner. Generic
/// click/activate verbs are never used, and `press` is allowed only for a
/// native GTK 3 combo box (L4-4), whose `Press` action pops down its showing
/// menu child rather than firing a button's primary handler.
pub(super) fn close_verbs(role: Option<u32>, toolkit: ToolkitKind) -> &'static [&'static str] {
    match (role, toolkit) {
        (Some(COMBO_BOX), ToolkitKind::Gtk3) => &GTK3_COMBO_DISMISS_VERBS,
        _ => &DISMISS_VERBS,
    }
}

pub(super) fn deepest_open_owner<'a>(
    node: &'a WalkedNode<ObservedRead>,
    depth: usize,
    best: &mut Option<(usize, &'a WalkedNode<ObservedRead>)>,
) {
    if node.in_web_document || is_web_document(role(node)) {
        return;
    }
    if is_open_owner(node) && best.is_none_or(|(deepest, _)| depth > deepest) {
        *best = Some((depth, node));
    }
    for child in &node.children {
        deepest_open_owner(child, depth + 1, best);
    }
}

fn frames_of(
    bus: &SharedBus,
    pid: u32,
    window: Option<&WindowInfo>,
    deadline: Deadline,
) -> Result<Vec<AtspiElement>, AdapterError> {
    let windows = match window {
        Some(window) => vec![window.clone()],
        None => windows_of_process(bus, pid, deadline)?.0,
    };
    let mut frames = Vec::new();
    for window in windows {
        let (name, path) = window_id::decode(&window.id)?;
        frames.push(rooted(&AtspiElement::new(name, path, pid)));
    }
    Ok(frames)
}

fn owner_toolkit(bus: &SharedBus, owner: &AtspiElement, deadline: Deadline) -> ToolkitKind {
    let Ok(connection) = bus.get(deadline) else {
        return ToolkitKind::Other;
    };
    let app = AtspiElement::new(owner.bus_name(), REGISTRY_ROOT_PATH, owner.pid());
    match within(deadline, read_toolkit(connection.inner(), &app)) {
        Ok(toolkit) => kind(&toolkit),
        Err(_) => ToolkitKind::Other,
    }
}

fn still_open(bus: &SharedBus, owner: &AtspiElement, deadline: Deadline) -> Option<bool> {
    let walk = scan_walk_structure(
        bus,
        &rooted(owner),
        OWNER_NODES,
        Some(POPUP_CONTAINER_CHILDREN),
        deadline,
    )
    .ok()?;
    popup_open(&walk)
}

/// Whether the owner's popup is open, read after a dismissal. `None` when the
/// owner's states, a child's role or a popup child's states could not be
/// read, or the walk was cut short: a popup that could not be read never
/// counts as closed.
pub(super) fn popup_open(owner: &WalkedNode<ObservedRead>) -> Option<bool> {
    if owner.children.iter().any(showing_popup) {
        return Some(true);
    }
    let unread = bits(owner).is_none()
        || owner.truncated
        || owner.children.iter().any(|child| match role(child) {
            Some(MENU | POPUP_MENU) => bits(child).is_none(),
            Some(_) => false,
            None => true,
        });
    (!unread).then_some(false)
}

/// What the owner's reply settles. A reply of false delivered nothing, so the
/// popup is not read and cannot turn that reply into a delivery.
fn popup_outcome(reply: WriteReply, read_closed: impl FnOnce() -> Option<bool>) -> DeliveryOutcome {
    match reply {
        WriteReply::Absent => DeliveryOutcome::NotDelivered,
        WriteReply::Done => DeliveryOutcome::observed(read_closed()),
    }
}

/// The answer for a refused Escape: the popup attempt's own result, or its own
/// error, which already says whether that attempt may have landed. The refusal
/// stands only when the target window shows no dismissible open popup owner.
pub(super) fn popup_answer(
    refusal: AdapterError,
    popup: Result<Option<ActionResult>, AdapterError>,
) -> Result<ActionResult, AdapterError> {
    match popup {
        Ok(Some(settled)) => Ok(settled),
        Ok(None) => Err(refusal),
        Err(attempt) => Err(attempt),
    }
}

/// The refused Escape of a background application, answered by closing the
/// open native popup its target window shows when the owner advertises a safe
/// dismissal verb. `Ok(None)` leaves the refusal as it was; every other
/// result, the popup attempt's own failure included, is the answer.
pub(super) fn close_open_popup(
    bus: &SharedBus,
    pid: u32,
    window: Option<&WindowInfo>,
    combo: &KeyCombo,
    deadline: Deadline,
) -> Result<Option<ActionResult>, AdapterError> {
    let mut best: Option<(usize, AtspiElement, Option<u32>)> = None;
    for frame in frames_of(bus, pid, window, deadline)? {
        let walk = scan_walk_structure(
            bus,
            &frame,
            SCAN_NODES,
            Some(POPUP_CONTAINER_CHILDREN),
            deadline,
        )?;
        let mut found = None;
        deepest_open_owner(&walk, 0, &mut found);
        if let Some((depth, node)) = found
            && best.as_ref().is_none_or(|(deepest, _, _)| depth > *deepest)
        {
            best = Some((depth, node.payload.node.element.clone(), role(node)));
        }
    }
    let Some((_, owner, owner_role)) = best else {
        return Ok(None);
    };
    let verbs = close_verbs(owner_role, owner_toolkit(bus, &owner, deadline));
    let handle = owner.clone().into_native_handle();
    let names = get_live_actions(bus, &handle, deadline)?.unwrap_or_default();
    if resolve_verb(&names, verbs).is_none() {
        return Ok(None);
    }
    let run = || {
        let target = Target {
            bus,
            handle: &handle,
            element: &owner,
            deadline,
        };
        let reply = do_action(&target, &names, verbs)?;
        Ok(popup_outcome(reply, || {
            deadline.poll_until(
                CLOSE_SETTLE,
                CLOSE_STEP,
                || still_open(bus, &owner, deadline).map(|open| !open),
                |closed| *closed == Some(true),
            )
        }))
    };
    let mut rungs = [ChainRung {
        label: CLOSE_LABEL,
        effects: &[],
        mechanism: StepMechanism::SemanticApi,
        run: Box::new(run),
    }];
    run_semantic_chain(
        &Action::PressKey(combo.clone()),
        &mut rungs,
        deadline,
        SUGGESTION,
    )
    .map(Some)
}

#[cfg(test)]
#[path = "escape_popup_tests.rs"]
mod tests;
