//! The effect witness of one element: what a click or a select can change
//! around it, read before and after the action.
//!
//! The target's own state comes from one bounded structure walk below it
//! (selected, and the popups that show below it) and the existing value
//! read. The window part comes from three reads in one step: the frame's name,
//! the frame's direct children and the application's frames. A field that the
//! reads cannot give stays `None`. The application's focused element is not
//! found cheaply on this platform, so the focus witness stays empty.

use std::hash::{DefaultHasher, Hash, Hasher};

use agent_desktop_core::{
    AdapterError, Deadline, EffectWitness, ErrorCode, NativeHandle, TargetWitness, WindowWitness,
};
use futures_util::future::join3;

use crate::system::{REGISTRY_ROOT_PATH, SharedBus, within};
use crate::tree::element::child_list::CHILD_LIST_LIMIT;
use crate::tree::menu::observe_menu::{rooted, scan_walk_structure};
use crate::tree::properties::liveness::Liveness;
use crate::tree::properties::node_read::{NAME, get_one, string_value};
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::vocabulary::role_numbers::{COMBO_BOX, MENU, POPUP_MENU, is_bulk_container};
use crate::tree::vocabulary::state_bits::{EXPANDED, HAS_POPUP, SELECTED, SHOWING, StateBits};
use crate::tree::walk::walker_expand::read_observed;
use crate::tree::{
    AtspiElement, ChildAddress, ObservedRead, WalkLimits, WalkedNode, atspi_element, list_children,
};

const TARGET_NODES: usize = 24;
const POPUP_CONTAINER_CHILDREN: usize = 16;

fn bits_of(read: &ObservedRead) -> Option<StateBits<'_>> {
    read.node
        .traits
        .states
        .present()
        .map(|words| StateBits::new(words))
}

fn bits(node: &WalkedNode<ObservedRead>) -> Option<StateBits<'_>> {
    bits_of(&node.payload)
}

/// The popup menus that show in a subtree, or `None` when a menu's states
/// could not be read or the walk stopped short of a node's children: an
/// uncounted popup would make a later whole read look like a popup the action
/// opened. A list, table or tree cut at its row limit hides only rows.
fn showing_popups(node: &WalkedNode<ObservedRead>) -> Option<u32> {
    let role = node.payload.node.traits.role.present().copied();
    if node.truncated && !is_bulk_container(role) {
        return None;
    }
    let own = match role {
        Some(MENU | POPUP_MENU) => u32::from(bits(node)?.has(SHOWING)),
        _ => 0,
    };
    node.children
        .iter()
        .map(showing_popups)
        .sum::<Option<u32>>()
        .map(|below| own + below)
}

/// Whether a target can own a popup below it: a combo box, or an element that
/// reports the popup or expanded state. Any other element has none, so the
/// subtree below it is not read.
pub(crate) fn may_own_popup(role: Option<u32>, bits: &StateBits<'_>) -> bool {
    role == Some(COMBO_BOX) || bits.has(HAS_POPUP) || bits.has(EXPANDED)
}

/// The popups open below the target: the popup menus that show in its
/// subtree, or one when the target itself reports `EXPANDED`.
pub(crate) fn surfaces_of(target: &WalkedNode<ObservedRead>) -> Option<u32> {
    let bits = bits(target)?;
    let shown = showing_popups(target)?;
    Some(if shown == 0 && bits.has(EXPANDED) {
        1
    } else {
        shown
    })
}

/// Whether the target's frame is still one of the application's frames, by
/// bus name and object path.
pub(crate) fn frame_listed(frames: &[ChildAddress], frame: &AtspiElement) -> bool {
    frames
        .iter()
        .any(|(bus, path)| bus == frame.bus_name() && path == frame.path())
}

/// A sign of a list of children that changes when one is added, removed or
/// replaced.
pub(crate) fn content_sign(children: &[ChildAddress]) -> u64 {
    let mut hasher = DefaultHasher::new();
    children.hash(&mut hasher);
    hasher.finish()
}

fn gone(error: AdapterError) -> AdapterError {
    match error.code {
        ErrorCode::ElementNotFound | ErrorCode::StaleRef | ErrorCode::AppNotFound => {
            AdapterError::stale_ref_because("The target element is gone")
        }
        _ => error,
    }
}

fn target_witness(
    bus: &SharedBus,
    element: &AtspiElement,
    deadline: Deadline,
) -> Result<TargetWitness, AdapterError> {
    let connection = bus.get(deadline)?;
    let (read, _) = within(deadline, async {
        read_observed(
            connection.inner(),
            element,
            WalkLimits::default().max_field_bytes,
            ReadPurpose::State,
            CHILD_LIST_LIMIT,
        )
        .await
    })
    .map_err(gone)?;
    let role = read.node.traits.role.present().copied();
    let surfaces = match bits_of(&read) {
        Some(bits) if may_own_popup(role, &bits) => scan_walk_structure(
            bus,
            &rooted(element),
            TARGET_NODES,
            Some(POPUP_CONTAINER_CHILDREN),
            deadline,
        )
        .map_err(gone)
        .ok()
        .and_then(|walk| surfaces_of(&walk)),
        Some(_) => Some(0),
        None => None,
    };
    Ok(TargetWitness {
        selected: bits_of(&read).map(|bits| bits.has(SELECTED)),
        value: read.value.value.present().cloned(),
        surfaces,
    })
}

fn window_witness(
    bus: &SharedBus,
    frame: &AtspiElement,
    deadline: Deadline,
) -> Result<WindowWitness, AdapterError> {
    let connection = bus.get(deadline)?;
    let application = AtspiElement::new(frame.bus_name(), REGISTRY_ROOT_PATH, frame.pid());
    let (name, children, frames) = within(deadline, async {
        Ok(join3(
            get_one(
                connection.inner(),
                frame,
                NAME,
                string_value,
                Liveness::Alive,
            ),
            list_children(connection.inner(), frame),
            list_children(connection.inner(), &application),
        )
        .await)
    })?;
    Ok(WindowWitness {
        title: name.ok().and_then(|name| name.present().cloned()),
        content: children.ok().map(|children| content_sign(&children)),
        present: frames
            .as_ref()
            .ok()
            .map(|frames| frame_listed(frames, frame)),
        windows: frames
            .ok()
            .and_then(|frames| u32::try_from(frames.len()).ok()),
    })
}

/// Reads the effect witness of the element `handle` names. A target that is
/// gone answers a stale ref, which core counts as the target gone.
pub(crate) fn read_effect_witness(
    bus: &SharedBus,
    handle: &NativeHandle,
    deadline: Deadline,
) -> Result<EffectWitness, AdapterError> {
    let element = atspi_element(handle)?;
    let target = target_witness(bus, element, deadline)?;
    let window = match element.frame() {
        Some(frame) => window_witness(bus, frame, deadline)?,
        None => WindowWitness::default(),
    };
    Ok(EffectWitness {
        target,
        window,
        ..EffectWitness::default()
    })
}

#[cfg(test)]
#[path = "effect_witness_tests.rs"]
mod tests;
