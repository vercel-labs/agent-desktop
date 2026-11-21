use agent_desktop_core::AdapterError;
use zbus::Connection;
use zbus::zvariant::OwnedValue;

use super::liveness::Liveness;
use super::node_read::{call, deserialize, field};
use crate::system::PROPERTIES_INTERFACE;
use crate::tree::element::child_view::ChildSlots;
use crate::tree::{AtspiElement, Field};

const TABLE_INTERFACE: &str = "org.a11y.atspi.Table";

/// What a node read learned about the grid it presents. `size` is the row and
/// column count of a node the platform reports as a table; `slots` describes
/// a child list that is only part of the node's children. A read that failed
/// leaves its fact unknown, never invented.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GridRead {
    pub(crate) slots: Option<ChildSlots>,
    pub(crate) size: Option<(u32, u32)>,
}

fn advertises(interfaces: &Field<Vec<String>>, name: &str) -> bool {
    interfaces
        .present()
        .is_some_and(|list| list.iter().any(|entry| entry == name))
}

async fn read_property<T>(
    connection: &Connection,
    element: &AtspiElement,
    interface: &str,
    name: &str,
    liveness: Liveness,
) -> Result<Option<T>, AdapterError>
where
    T: TryFrom<OwnedValue>,
{
    let reply = call(
        connection,
        element,
        PROPERTIES_INTERFACE,
        "Get",
        &(interface, name),
    )
    .await;
    Ok(field(deserialize::<OwnedValue>(reply), liveness)?
        .present()
        .and_then(|value| T::try_from(value.try_clone().ok()?).ok()))
}

fn unsigned(value: i32) -> Option<u32> {
    u32::try_from(value).ok()
}

async fn read_size(
    connection: &Connection,
    element: &AtspiElement,
    liveness: Liveness,
) -> Result<Option<(u32, u32)>, AdapterError> {
    let rows: Option<i32> =
        read_property(connection, element, TABLE_INTERFACE, "NRows", liveness).await?;
    let columns: Option<i32> =
        read_property(connection, element, TABLE_INTERFACE, "NColumns", liveness).await?;
    Ok(rows.and_then(unsigned).zip(columns.and_then(unsigned)))
}

/// The table size of one node, read only when the node advertises the `Table`
/// interface, so any other node costs no call.
pub(in crate::tree) async fn read_grid_facts(
    connection: &Connection,
    element: &AtspiElement,
    interfaces: &Field<Vec<String>>,
    liveness: Liveness,
) -> Result<GridRead, AdapterError> {
    let size = if advertises(interfaces, TABLE_INTERFACE) {
        read_size(connection, element, liveness).await?
    } else {
        None
    };
    Ok(GridRead { slots: None, size })
}
