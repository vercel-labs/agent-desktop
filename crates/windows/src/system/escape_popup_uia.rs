//! The UI Automation half of `escape_popup`: the bounded popup search, the collapse and the entry point.

use super::{
    MAX_POPUP_ROOTS, PopupScan, PopupTree, opens_a_popup, scan_popup_owners, scan_timeout_error,
    verify_process_handle, verify_process_target, verify_window_target,
};
use agent_desktop_core::{
    Action, ActionResult, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    KeyCombo, ProcessIdentity, WindowInfo,
};
use uiautomation::core::{UICacheRequest, UITreeWalker};

use crate::actions::disclosure::collapse_with_guard;
use crate::system::window::window_enum::{WindowHandle, enumerate_top_level};
use crate::system::window::window_identity::live_window_owner;
use crate::tree::automation::{automation_client, root_from_hwnd};
use crate::tree::element::UIAElement;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::properties::read_cached;
use crate::tree::walk::cache::build_walk_cache_request;

const EXPANDED: i32 = 1;
const POPUP_SCAN_PROPERTIES: [TreeProperty; 2] =
    [TreeProperty::ControlType, TreeProperty::ExpandCollapseState];

struct PopupNode {
    handle: WindowHandle,
    element: UIAElement,
}

struct UiaPopupTree {
    walker: UITreeWalker,
    cache: UICacheRequest,
}

impl PopupTree for UiaPopupTree {
    type Node = PopupNode;

    fn is_open_popup_owner(&self, node: &PopupNode) -> bool {
        let (properties, _) = read_cached(&node.element, &POPUP_SCAN_PROPERTIES);
        properties
            .get(TreeProperty::ControlType)
            .number()
            .is_some_and(opens_a_popup)
            && properties.get(TreeProperty::ExpandCollapseState).number() == Some(EXPANDED)
    }

    fn first_child(&self, node: &PopupNode) -> Option<PopupNode> {
        self.walker
            .get_first_child_build_cache(&node.element.0, &self.cache)
            .ok()
            .map(|element| PopupNode {
                handle: node.handle,
                element: UIAElement::from(element),
            })
    }

    fn next_sibling(&self, node: &PopupNode) -> Option<PopupNode> {
        self.walker
            .get_next_sibling_build_cache(&node.element.0, &self.cache)
            .ok()
            .map(|element| PopupNode {
                handle: node.handle,
                element: UIAElement::from(element),
            })
    }
}

fn prefetch_roots(
    roots: Vec<PopupNode>,
    cache: &UICacheRequest,
    deadline: Deadline,
) -> Result<Vec<PopupNode>, AdapterError> {
    let mut cached = Vec::with_capacity(roots.len());
    for root in roots.into_iter().take(MAX_POPUP_ROOTS) {
        if deadline.is_expired() {
            return Err(scan_timeout_error(deadline));
        }
        if let Ok(element) = root.element.0.build_updated_cache(cache) {
            cached.push(PopupNode {
                handle: root.handle,
                element: UIAElement::from(element),
            });
        }
    }
    Ok(cached)
}

/// The deepest expanded popup owner among the elements below `roots`, found by
/// a bounded, deadline-checked incremental walk over the control view.
fn deepest_popup_owner(
    roots: Vec<PopupNode>,
    deadline: Deadline,
) -> Result<Option<PopupNode>, AdapterError> {
    if deadline.is_expired() {
        return Err(scan_timeout_error(deadline));
    }
    let Ok(client) = automation_client() else {
        return Ok(None);
    };
    let Ok(walker) = client.get_control_view_walker() else {
        return Ok(None);
    };
    let Ok(cache) = build_walk_cache_request(&client, &POPUP_SCAN_PROPERTIES) else {
        return Ok(None);
    };
    let cached_roots = prefetch_roots(roots, &cache, deadline)?;
    let tree = UiaPopupTree { walker, cache };
    match scan_popup_owners(&tree, cached_roots, deadline) {
        PopupScan::Found(owner) => Ok(Some(owner)),
        PopupScan::None => Ok(None),
        PopupScan::TimedOut => Err(scan_timeout_error(deadline)),
    }
}

fn window_roots(window: &WindowInfo, deadline: Deadline) -> Result<Vec<PopupNode>, AdapterError> {
    if deadline.is_expired() {
        return Err(scan_timeout_error(deadline));
    }
    let handle = verify_window_target(window)?;
    let element = match root_from_hwnd(handle as isize, deadline) {
        Ok(element) => element,
        Err(error) => {
            verify_window_target(window)?;
            if error.code == ErrorCode::Timeout {
                return Err(error.with_disposition(DeliverySemantics::not_delivered()));
            }
            return Ok(Vec::new());
        }
    };
    verify_window_target(window)?;
    Ok(vec![PopupNode { handle, element }])
}

fn process_roots(
    process: &ProcessIdentity,
    deadline: Deadline,
) -> Result<Vec<PopupNode>, AdapterError> {
    if deadline.is_expired() {
        return Err(scan_timeout_error(deadline));
    }
    verify_process_target(process)?;
    let mut handles = Vec::new();
    let _ = enumerate_top_level(|window| {
        if live_window_owner(window.handle) == Some(process.pid) {
            handles.push(window.handle);
        }
        handles.len() < MAX_POPUP_ROOTS
    });
    let mut roots = Vec::with_capacity(handles.len());
    for handle in handles {
        if deadline.is_expired() {
            return Err(scan_timeout_error(deadline));
        }
        if verify_process_handle(process, handle).is_err() {
            verify_process_target(process)?;
            continue;
        }
        if let Ok(element) = root_from_hwnd(handle as isize, deadline) {
            if verify_process_handle(process, handle).is_ok() {
                roots.push(PopupNode { handle, element });
            } else {
                verify_process_target(process)?;
            }
        }
    }
    Ok(roots)
}

/// Collapses the deepest open popup of the window (or of the process), and
/// reads its state back.
pub(crate) fn close_transient_surface(
    process: &ProcessIdentity,
    window: Option<&WindowInfo>,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<Option<ActionResult>, AdapterError> {
    let roots = match window {
        Some(window) => window_roots(window, deadline)?,
        None => process_roots(process, deadline)?,
    };
    let Some(owner) = deepest_popup_owner(roots, deadline)? else {
        return Ok(None);
    };
    if deadline.is_expired() {
        return Err(scan_timeout_error(deadline));
    }
    let verify = || match window {
        Some(window) => verify_window_target(window).map(|_| ()),
        None => verify_process_handle(process, owner.handle),
    };
    let steps = collapse_with_guard(&owner.element, policy, deadline, &verify)?;
    let escape = KeyCombo {
        key: "escape".to_owned(),
        modifiers: Vec::new(),
    };
    Ok(Some(ActionResult::from_execution(
        &Action::PressKey(escape),
        steps,
    )))
}
