use agent_desktop_core::{ActionResult, AdapterError, Deadline};

use super::select_search::{Container, Hit, ItemReader};
use super::{BusReader, handle_of, parent_of, select_through};
use crate::actions::context::Target;
use crate::system::{ACCESSIBLE_INTERFACE, SharedBus, within};
use crate::tree::AtspiElement;
use crate::tree::capabilities::action_read::{NodeView, read_node_view};
use crate::tree::vocabulary::role_numbers::TABLE_CELL;
use crate::tree::vocabulary::state_bits::TRANSIENT;

/// Whether `parent` has `node` at the index `node` reports in its parent:
/// one `GetIndexInParent` and one `GetChildAtIndex`, so a parent that
/// reports millions of children is never listed.
fn is_child_at_own_index(
    bus: &SharedBus,
    parent: &AtspiElement,
    node: &AtspiElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let connection = bus.get(deadline)?;
    let transient = read_node_view(bus, &handle_of(node), deadline)
        .is_ok_and(|view| view.state(TRANSIENT) == Some(true));
    within(deadline, async {
        let reply = crate::tree::call(
            connection.inner(),
            node,
            ACCESSIBLE_INTERFACE,
            "GetIndexInParent",
            &(),
        )
        .await;
        let Some(index) = reply
            .ok()
            .and_then(|reply| reply.body().deserialize::<i32>().ok())
            .and_then(|index| usize::try_from(index).ok())
        else {
            return Ok(false);
        };
        let child = crate::tree::child_at_index(connection.inner(), parent, index).await?;
        Ok(child.is_some_and(|(bus, path)| {
            (bus == node.bus_name() && path == node.path()) || transient
        }))
    })
}

pub(super) fn selection_parent(
    reader: &mut BusReader<'_>,
    node: &AtspiElement,
) -> Result<Option<Container<AtspiElement>>, AdapterError> {
    let Some(parent) = parent_of(reader.bus, node, reader.deadline)? else {
        return Ok(None);
    };
    if !reader.inspect(&parent)?.selection {
        return Ok(None);
    }
    let is_cell = read_node_view(reader.bus, &handle_of(node), reader.deadline)
        .is_ok_and(|view| view.role() == Some(TABLE_CELL));
    let present =
        if is_cell {
            is_child_at_own_index(reader.bus, &parent, node, reader.deadline)?
        } else {
            reader.children(&parent)?.0.iter().any(|sibling| {
                sibling.bus_name() == node.bus_name() && sibling.path() == node.path()
            })
        };
    Ok(present.then_some(Container { node: parent }))
}

/// The last rung of a click on a node that names no action of its own: the
/// parent that advertises `Selection` selects it. `None` when the node has
/// no such parent, so the click stays refused. The result is the selection
/// of the node, checked by its `selected` state, and, as for a select, no
/// other child of the parent stays selected.
pub(crate) fn select_by_parent(
    target: &Target<'_>,
    view: &NodeView,
) -> Result<Option<ActionResult>, AdapterError> {
    let mut reader = BusReader {
        bus: target.bus,
        deadline: target.deadline,
        inspected_role: None,
    };
    let Some(container) = selection_parent(&mut reader, target.element)? else {
        return Ok(None);
    };
    let hit = Hit {
        item: target.element.clone(),
        container: Some(container.clone()),
        row: None,
        own: false,
    };
    let display_value = view.name().unwrap_or_default().to_owned();
    select_through(target, &hit, Some(&container), &display_value).map(Some)
}
