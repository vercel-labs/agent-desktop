//! Escape for an open popup when the key route refused for focus.
//!
//! The key route needs the target to hold the keyboard. A popup a person
//! opened (a menu, a drop-down, a split-button list) can sit in a window that
//! is not the foreground window, and the key is then refused. The popup still
//! has a semantic close: `ExpandCollapsePattern.Collapse` on its owner. The
//! states, in this order: the key route delivers, unchanged; it refuses for
//! focus and the target shows an open popup, whose owner is collapsed and read
//! back; otherwise the refusal stays as it was.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, KeyCombo};

#[cfg(any(target_os = "windows", test))]
pub(super) const MAX_POPUP_ROOTS: usize = 32;
#[cfg(any(target_os = "windows", test))]
pub(super) const MAX_POPUP_NODES: usize = 600;
#[cfg(any(target_os = "windows", test))]
pub(super) const MAX_POPUP_DEPTH: usize = 32;
#[cfg(any(target_os = "windows", test))]
pub(super) const MAX_POPUP_CHILDREN: usize = 64;

/// Outcome of a bounded background-Escape popup scan.
#[cfg(any(target_os = "windows", test))]
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PopupScan<N> {
    Found(N),
    None,
    TimedOut,
}

/// Incremental tree reader used by [`scan_popup_owners`] so the bounded walk
/// is tested without live UI Automation.
#[cfg(any(target_os = "windows", test))]
pub(super) trait PopupTree {
    type Node;

    fn is_open_popup_owner(&self, node: &Self::Node) -> bool;
    fn first_child(&self, node: &Self::Node) -> Option<Self::Node>;
    fn next_sibling(&self, node: &Self::Node) -> Option<Self::Node>;
}

#[cfg(any(target_os = "windows", test))]
pub(super) fn scan_timeout_error(deadline: Deadline) -> AdapterError {
    deadline
        .timeout_error()
        .with_disposition(DeliverySemantics::not_delivered())
}

/// Verifies that `window` still names a live handle owned by the stored process
/// generation, failing closed as not delivered when the handle is malformed,
/// lacks a process-instance token, or no longer matches its owner.
#[cfg(any(target_os = "windows", test))]
pub(super) fn verify_window_target(
    window: &agent_desktop_core::WindowInfo,
) -> Result<crate::system::window::window_enum::WindowHandle, AdapterError> {
    if window.is_process_owner() {
        return Err(AdapterError::app_has_no_window(&window.app)
            .with_disposition(DeliverySemantics::not_delivered()));
    }
    let handle = crate::system::window::window_ops::parse_handle(&window.id);
    if handle.is_null() {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "window id is not a Windows HWND-shaped id",
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    let Some(evidence) =
        crate::system::window::window_identity::WindowIdentityEvidence::from_info(handle, window)
    else {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "popup dismissal requires a process-instance token on the stored window",
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    };
    evidence
        .verify_stored()
        .map_err(|error| error.with_disposition(DeliverySemantics::not_delivered()))?;
    Ok(handle)
}

/// Verifies that `process` still names the same running process generation,
/// failing closed as not delivered when the process exited or its pid recycled.
#[cfg(any(target_os = "windows", test))]
pub(super) fn verify_process_target(
    process: &agent_desktop_core::ProcessIdentity,
) -> Result<(), AdapterError> {
    if crate::system::process_identity::matches_instance(process.pid, &process.instance)? {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Target process instance is no longer running",
    )
    .with_details(serde_json::json!({
        "pid": process.pid,
        "process_instance": process.instance,
        "complete": false,
        "physical_delivery_started": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered()))
}

/// Verifies that `handle` is still owned by `process` in its stored generation.
#[cfg(any(target_os = "windows", test))]
pub(super) fn verify_process_handle(
    process: &agent_desktop_core::ProcessIdentity,
    handle: crate::system::window::window_enum::WindowHandle,
) -> Result<(), AdapterError> {
    verify_process_target(process)?;
    if crate::system::window::window_identity::live_window_owner(handle) == Some(process.pid) {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::WindowNotFound,
        "The popup window is no longer owned by the target process",
    )
    .with_suggestion("Run 'list-windows' to refresh window identifiers, then retry.")
    .with_disposition(DeliverySemantics::not_delivered()))
}

/// Finds the deepest open popup owner across at most `MAX_POPUP_ROOTS` roots
/// and `MAX_POPUP_NODES` nodes, consulting `deadline` before every step.
#[cfg(any(target_os = "windows", test))]
pub(super) fn scan_popup_owners<T: PopupTree>(
    tree: &T,
    roots: Vec<T::Node>,
    deadline: Deadline,
) -> PopupScan<T::Node> {
    if deadline.is_expired() {
        return PopupScan::TimedOut;
    }
    let mut visited = 0usize;
    let mut best: Option<(usize, T::Node)> = None;
    for root in roots.into_iter().take(MAX_POPUP_ROOTS) {
        let mut stack = vec![(root, 0usize)];
        while let Some((node, depth)) = stack.pop() {
            if deadline.is_expired() {
                return PopupScan::TimedOut;
            }
            if visited >= MAX_POPUP_NODES {
                return best.map_or(PopupScan::None, |(_, owner)| PopupScan::Found(owner));
            }
            visited += 1;
            let is_owner = tree.is_open_popup_owner(&node);
            if depth < MAX_POPUP_DEPTH && visited < MAX_POPUP_NODES {
                let mut children = Vec::new();
                if deadline.is_expired() {
                    return PopupScan::TimedOut;
                }
                let mut child = tree.first_child(&node);
                while let Some(current) = child {
                    let next = if children.len() + 1 < MAX_POPUP_CHILDREN {
                        if deadline.is_expired() {
                            return PopupScan::TimedOut;
                        }
                        tree.next_sibling(&current)
                    } else {
                        None
                    };
                    children.push(current);
                    child = next;
                }
                for child in children.into_iter().rev() {
                    stack.push((child, depth + 1));
                }
            }
            if is_owner && best.as_ref().is_none_or(|(deepest, _)| depth >= *deepest) {
                best = Some((depth, node));
            }
        }
    }
    best.map_or(PopupScan::None, |(_, owner)| PopupScan::Found(owner))
}

/// Key delivery to `process` (A9-3: SendInput has no per-pid targeting;
/// core's headed path already activated), then the popup close.
pub(crate) fn press_for_app(
    process: agent_desktop_core::ProcessIdentity,
    combo: &KeyCombo,
    policy: agent_desktop_core::InteractionPolicy,
    deadline: agent_desktop_core::Deadline,
) -> Result<agent_desktop_core::ActionResult, AdapterError> {
    let result =
        crate::system::key_dispatch::press_for_app_impl(process.clone(), combo, policy, deadline);
    after_key_route(
        result,
        PopupScope::Process(&process),
        combo,
        policy,
        deadline,
    )
}

/// Core's focus-gated delivery to a named window, then the popup close.
pub(crate) fn press_for_window<A: agent_desktop_core::SystemOps>(
    adapter: &A,
    window: &agent_desktop_core::WindowInfo,
    combo: &KeyCombo,
    policy: agent_desktop_core::InteractionPolicy,
    lease: &agent_desktop_core::InteractionLease,
) -> Result<agent_desktop_core::ActionResult, AdapterError> {
    let result =
        agent_desktop_core::press_key_in_focused_window(adapter, window, combo, policy, lease);
    after_key_route(
        result,
        PopupScope::Window(window),
        combo,
        policy,
        lease.deadline(),
    )
}

/// Where a refused Escape looks for an open popup.
#[derive(Clone, Copy)]
pub(crate) enum PopupScope<'a> {
    Window(&'a agent_desktop_core::WindowInfo),
    Process(&'a agent_desktop_core::ProcessIdentity),
}

pub(crate) fn is_plain_escape(combo: &KeyCombo) -> bool {
    combo.modifiers.is_empty()
        && (combo.key.eq_ignore_ascii_case("escape") || combo.key.eq_ignore_ascii_case("esc"))
}

/// The refusals after which the target's popup gets its close: core's refusal
/// of a window that is not the exact focus holder, and this adapter's refusal
/// of a process that does not hold the keyboard. Each scope answers only its
/// own refusal, so one refusal is searched once.
pub(crate) fn refused_for_focus(window_scope: bool, error: &AdapterError) -> bool {
    let details = error.details.as_ref();
    let unsent = error.disposition == DeliverySemantics::not_delivered()
        && details.and_then(|details| details["physical_delivery_started"].as_bool())
            == Some(false);
    let check = details.and_then(|details| details["check"].as_str());
    unsent
        && if window_scope {
            check == Some("window_active")
        } else {
            error.code == ErrorCode::ActionFailed && check.is_none()
        }
}

/// The control types whose expansion is a popup: menu item (50011), combo box
/// (50003) and split button (50031). A tree item or a group is never one.
fn opens_a_popup(control_type: i32) -> bool {
    [50011, 50003, 50031].contains(&control_type)
}

#[cfg(target_os = "windows")]
#[path = "escape_popup_uia.rs"]
mod uia;

#[cfg(target_os = "windows")]
pub(crate) use uia::after_key_route;

/// Off Windows no popup can be open, so the key route's result stands.
#[cfg(not(target_os = "windows"))]
pub(crate) fn after_key_route(
    result: Result<agent_desktop_core::ActionResult, AdapterError>,
    _scope: PopupScope<'_>,
    _combo: &KeyCombo,
    _policy: agent_desktop_core::InteractionPolicy,
    _deadline: agent_desktop_core::Deadline,
) -> Result<agent_desktop_core::ActionResult, AdapterError> {
    result
}

#[cfg(test)]
#[path = "escape_popup_tests.rs"]
mod tests;
