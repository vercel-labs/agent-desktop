use agent_desktop_core::{LiveElement, tree::state};

use super::NodeReader;
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::element::ancestry::Ancestry;
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind};
use crate::tree::vocabulary::role_numbers::is_menu_item;
use crate::tree::vocabulary::state_bits::{SHOWING, StateBits, VISIBLE};
use crate::tree::{AtspiElement, ObservedRead};

fn bits(read: &ObservedRead) -> Option<StateBits<'_>> {
    read.node
        .traits
        .states
        .present()
        .map(|words| StateBits::new(words))
}

/// Whether a Qt menu item is hidden only because its menu is closed while the
/// menu bar entry that owns the menu is showing. Qt answers `Press` on such an
/// item without opening the menu or touching the foreground (wave0 C2: Kate
/// 2/2, KCalc 1/2, zero sentinel events), so it is as reachable as a closed
/// menu item on any platform. The menu bar entry is the first menu ancestor
/// the walk recorded, because a menu bar resets that list.
fn answers_while_closed<R: NodeReader>(
    reader: &mut R,
    element: &AtspiElement,
    read: &ObservedRead,
    toolkit: &ToolkitId,
) -> bool {
    let item_is_closed = read
        .node
        .traits
        .role
        .present()
        .copied()
        .is_some_and(is_menu_item)
        && bits(read).is_some_and(|bits| !bits.has(VISIBLE) && !bits.has(SHOWING));
    if !item_is_closed || kind(toolkit) != ToolkitKind::Qt {
        return false;
    }
    let Some(mut entry) = element.ancestry().menu_ancestors().first().cloned() else {
        return false;
    };
    entry.set_ancestry(Ancestry::of(element.frame().cloned(), None, false));
    reader
        .read(&entry)
        .is_ok_and(|entry| bits(&entry.read).is_some_and(|bits| bits.has(SHOWING)))
}

/// Projects a closed Qt menu item the way a closed menu item reads on every
/// platform: not hidden, with no claim about being on screen, and its bounds
/// kept. The `hidden` state token stays, so a snapshot still shows it closed.
pub(super) fn reach_closed_menu_item<R: NodeReader>(
    reader: &mut R,
    element: &AtspiElement,
    read: &ObservedRead,
    toolkit: &ToolkitId,
    live: &mut LiveElement,
) {
    if answers_while_closed(reader, element, read, toolkit) {
        live.state.hidden = Some(false);
        live.state.offscreen = None;
        live.state.states.retain(|token| token != state::OFFSCREEN);
    }
}

#[cfg(test)]
#[path = "closed_menu_tests.rs"]
mod tests;
