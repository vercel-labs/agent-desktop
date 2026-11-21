use agent_desktop_core::{AdapterError, ErrorCode};
use zbus::Connection;
use zbus::zvariant::OwnedObjectPath;

use crate::system::ACCESSIBLE_INTERFACE;
use crate::tree::properties::liveness::Liveness;
use crate::tree::properties::node_read::{
    CHILD_COUNT, NULL_PATH, call, count_value, deserialize, field, get_one, read_children,
};
use crate::tree::{AtspiElement, ChildAddress, Field};

/// The most children one object may be asked to list in one `GetChildren`
/// call. An object that reports more than this many children is never
/// listed: a toolkit that manages its descendants, such as a spreadsheet,
/// reports millions and builds an object for each one when it is asked for
/// the list, and no deadline of the caller can cancel that work.
pub(crate) const CHILD_LIST_LIMIT: usize = 4_096;

/// Whether a reported `ChildCount` is above `limit`. A count that is not
/// known, or is negative, is never above it.
pub(crate) fn exceeds(count: &Field<i32>, limit: usize) -> Option<i32> {
    match count {
        Field::Present(count) if usize::try_from(*count).is_ok_and(|count| count > limit) => {
            Some(*count)
        }
        Field::Present(_) | Field::Absent | Field::Failed(_) => None,
    }
}

/// The children of an object as `GetChildren` lists them, with the null
/// placeholder entries removed. Every caller that wants a whole child list
/// goes through this function, which refuses an object whose reported count
/// is above `CHILD_LIST_LIMIT` before the application is asked for the list.
/// An object that does not report a count is listed as before.
pub(crate) async fn list_children(
    connection: &Connection,
    element: &AtspiElement,
) -> Result<Vec<ChildAddress>, AdapterError> {
    let count = get_one(
        connection,
        element,
        CHILD_COUNT,
        count_value,
        Liveness::Alive,
    )
    .await?;
    if let Some(count) = exceeds(&count, CHILD_LIST_LIMIT) {
        return Err(AdapterError::new(
            ErrorCode::Internal,
            format!("The accessible object reports {count} children, more than one read lists"),
        ));
    }
    match read_listing(connection, element, Liveness::Alive).await? {
        Field::Present(children) => Ok(children),
        Field::Absent => Ok(Vec::new()),
        Field::Failed(detail) => Err(AdapterError::new(
            ErrorCode::Internal,
            "The children of the accessible object could not be listed",
        )
        .with_platform_detail(detail)),
    }
}

/// The only `GetChildren` call of a node read. Callers have checked the
/// object's reported count against `CHILD_LIST_LIMIT` first.
pub(in crate::tree) async fn read_listing(
    connection: &Connection,
    element: &AtspiElement,
    liveness: Liveness,
) -> Result<Field<Vec<ChildAddress>>, AdapterError> {
    let reply = call(
        connection,
        element,
        ACCESSIBLE_INTERFACE,
        "GetChildren",
        &(),
    )
    .await;
    read_children(reply, liveness)
}

/// The one child at `index`, read with `GetChildAtIndex`. `None` when the
/// object has no child there or the call failed for that child.
pub(crate) async fn child_at_index(
    connection: &Connection,
    element: &AtspiElement,
    index: usize,
) -> Result<Option<ChildAddress>, AdapterError> {
    let Ok(index) = i32::try_from(index) else {
        return Ok(None);
    };
    let reply = call(
        connection,
        element,
        ACCESSIBLE_INTERFACE,
        "GetChildAtIndex",
        &(index,),
    )
    .await;
    let read = field(
        deserialize::<(String, OwnedObjectPath)>(reply),
        Liveness::Alive,
    )?;
    Ok(match read {
        Field::Present((bus, path)) if !bus.is_empty() && path.as_str() != NULL_PATH => {
            Some((bus, path.to_string()))
        }
        Field::Present(_) | Field::Absent | Field::Failed(_) => None,
    })
}
