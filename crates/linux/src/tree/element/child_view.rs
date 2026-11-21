//! What an object presents as its children when it is read for a walk.
//!
//! An object that reports no more children than the limit is listed with one
//! `GetChildren` call. A larger object is never listed. One that has the
//! `Table` interface presents the cells in view, found from the screen
//! positions of its own corners; any other presents its first children, read
//! one by one, and so does a table without the `Component` interface, which
//! has no extents to find a view in. Every child of a larger object keeps its index in the parent,
//! so a saved path to it resolves with `GetChildAtIndex`.

use agent_desktop_core::AdapterError;
use futures_util::stream::{self, StreamExt};
use zbus::Connection;

use super::child_list::{CHILD_LIST_LIMIT, child_at_index, exceeds, read_listing};
use super::table_view::{TABLE_INTERFACE, read_table_view};
use crate::system::ACCESSIBLE_INTERFACE;
use crate::tree::properties::liveness::Liveness;
use crate::tree::properties::node_read::{
    CHILD_COUNT, COMPONENT_INTERFACE, call, count_value, deserialize, field, get_one,
};
use crate::tree::{AtspiElement, ChildAddress, Field, NodeProperties};

const CHILDREN_IN_FLIGHT: usize = 32;

/// How a child list that is shorter than the object's own count was read.
/// `positions` holds the index in the parent of each listed child, in the
/// order of the list; `reported` is the count the object gave. `grid_view` is
/// set when the list is the cells in view of a table: the snapshot presents
/// that view as complete, while the count it carries tells core that cells
/// were left unread, so no ref is proven unique against them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildSlots {
    pub(crate) positions: Vec<usize>,
    pub(crate) reported: u32,
    pub(crate) grid_view: bool,
}

pub(in crate::tree) struct ChildView {
    pub(in crate::tree) children: Field<Vec<ChildAddress>>,
    pub(in crate::tree) slots: Option<ChildSlots>,
}

impl ChildView {
    pub(in crate::tree) fn not_requested() -> Self {
        Self {
            children: Field::Failed("field not requested".to_owned()),
            slots: None,
        }
    }
}

/// The object's reported child count: the value the node read already holds,
/// or one `ChildCount` read when the purpose of the read did not ask for it.
pub(in crate::tree) async fn child_count_of(
    connection: &Connection,
    element: &AtspiElement,
    properties: &NodeProperties,
    liveness: Liveness,
) -> Result<Field<i32>, AdapterError> {
    match &properties.child_count {
        Field::Present(count) => Ok(Field::Present(*count)),
        Field::Absent | Field::Failed(_) => {
            get_one(connection, element, CHILD_COUNT, count_value, liveness).await
        }
    }
}

/// The children one node read presents, at most `limit` of them listed.
pub(in crate::tree) async fn read_child_view(
    connection: &Connection,
    element: &AtspiElement,
    count: &Field<i32>,
    interfaces: &Field<Vec<String>>,
    limit: usize,
    liveness: Liveness,
) -> Result<ChildView, AdapterError> {
    let Some(reported) = exceeds(count, limit) else {
        return Ok(ChildView {
            children: read_listing(connection, element, liveness).await?,
            slots: None,
        });
    };
    let reported = u32::try_from(reported).unwrap_or(u32::MAX);
    let names = interface_names(connection, element, interfaces, liveness).await?;
    let has = |wanted: &str| names.iter().any(|name| name == wanted);
    let is_table = has(TABLE_INTERFACE) && has(COMPONENT_INTERFACE);
    if is_table
        && let Some((cells, whole)) = read_table_view(connection, element, limit, liveness).await?
    {
        let (children, positions) = cells.into_iter().unzip();
        return Ok(ChildView {
            children: Field::Present(children),
            slots: Some(ChildSlots {
                positions,
                reported,
                grid_view: whole,
            }),
        });
    }
    leading_children(connection, element, reported, limit).await
}

/// The children a descent along a stored path needs from one node. A node
/// that reports more children than `limit` gives the single child at `toward`
/// when the path names one, read with `GetChildAtIndex`; every other node
/// presents what `read_child_view` presents.
pub(in crate::tree) async fn read_level_children(
    connection: &Connection,
    element: &AtspiElement,
    count: &Field<i32>,
    toward: Option<usize>,
    liveness: Liveness,
) -> Result<ChildView, AdapterError> {
    if let (Some(reported), Some(index)) = (exceeds(count, CHILD_LIST_LIMIT), toward) {
        let child = child_at_index(connection, element, index).await?;
        let (children, positions): (Vec<_>, Vec<_>) =
            child.into_iter().map(|child| (child, index)).unzip();
        return Ok(ChildView {
            children: Field::Present(children),
            slots: Some(ChildSlots {
                positions,
                reported: u32::try_from(reported).unwrap_or(u32::MAX),
                grid_view: true,
            }),
        });
    }
    let unknown = Field::Failed("interfaces not read".to_owned());
    read_child_view(
        connection,
        element,
        count,
        &unknown,
        CHILD_LIST_LIMIT,
        liveness,
    )
    .await
}

/// The children an object presents, for a caller that follows the live tree
/// and has no stored path: the whole list of an object within the limit, the
/// cells in view of a larger table, the leading children of any other larger
/// object. The flag says whether that is every child. The hit test needs only
/// the objects that can sit under a point; a caller that concludes anything
/// about all children, as a search by name does, must not conclude it from a
/// list that is not whole.
pub(crate) async fn presented_children(
    connection: &Connection,
    element: &AtspiElement,
) -> Result<(Vec<ChildAddress>, bool), AdapterError> {
    let count = get_one(
        connection,
        element,
        CHILD_COUNT,
        count_value,
        Liveness::Alive,
    )
    .await?;
    let unknown = Field::Failed("interfaces not read".to_owned());
    let view = read_child_view(
        connection,
        element,
        &count,
        &unknown,
        CHILD_LIST_LIMIT,
        Liveness::Alive,
    )
    .await?;
    match view.children {
        Field::Present(children) => {
            let whole = view
                .slots
                .is_none_or(|slots| slots.reported as usize <= children.len());
            Ok((children, whole))
        }
        Field::Absent => Ok((Vec::new(), true)),
        Field::Failed(detail) => Err(AdapterError::new(
            agent_desktop_core::ErrorCode::Internal,
            "The children of the accessible object could not be listed",
        )
        .with_platform_detail(detail)),
    }
}

/// Whether `parent` presents `node` at the index `node` reports for itself,
/// read with one `GetIndexInParent` and one `GetChildAtIndex`. A failed or
/// disagreeing read is `false`, so no list of the parent is needed.
pub(crate) async fn is_child_at_own_index(
    connection: &Connection,
    parent: &AtspiElement,
    node: &AtspiElement,
) -> Result<bool, AdapterError> {
    let reply = call(
        connection,
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
    let child = child_at_index(connection, parent, index).await?;
    Ok(child.is_some_and(|(bus, path)| bus == node.bus_name() && path == node.path()))
}

/// The interface names of an object, as one `GetInterfaces` call gives them.
pub(in crate::tree) async fn read_interfaces(
    connection: &Connection,
    element: &AtspiElement,
    liveness: Liveness,
) -> Result<Field<Vec<String>>, AdapterError> {
    let reply = call(
        connection,
        element,
        ACCESSIBLE_INTERFACE,
        "GetInterfaces",
        &(),
    )
    .await;
    field(deserialize::<Vec<String>>(reply), liveness)
}

async fn interface_names(
    connection: &Connection,
    element: &AtspiElement,
    known: &Field<Vec<String>>,
    liveness: Liveness,
) -> Result<Vec<String>, AdapterError> {
    if let Field::Present(names) = known {
        return Ok(names.clone());
    }
    Ok(
        match read_interfaces(connection, element, liveness).await? {
            Field::Present(names) => names,
            Field::Absent | Field::Failed(_) => Vec::new(),
        },
    )
}

async fn leading_children(
    connection: &Connection,
    element: &AtspiElement,
    reported: u32,
    limit: usize,
) -> Result<ChildView, AdapterError> {
    let wanted = usize::try_from(reported).map_or(limit, |count| count.min(limit));
    let reads: Vec<Result<Option<ChildAddress>, AdapterError>> =
        stream::iter((0..wanted).map(|index| child_at_index(connection, element, index)))
            .buffered(CHILDREN_IN_FLIGHT)
            .collect()
            .await;
    let mut children = Vec::new();
    let mut positions = Vec::new();
    for (index, read) in reads.into_iter().enumerate() {
        if let Some(child) = read? {
            children.push(child);
            positions.push(index);
        }
    }
    Ok(ChildView {
        children: Field::Present(children),
        slots: Some(ChildSlots {
            positions,
            reported,
            grid_view: false,
        }),
    })
}

#[cfg(test)]
#[path = "child_view_tests.rs"]
mod tests;
