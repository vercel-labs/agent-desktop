//! An in-memory table on a private bus: `columns` cells per row, laid out
//! row-major in a view of `view_rows` x `columns` cells of 10 x 10 pixels at
//! the screen origin. It counts every call a read makes.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zbus::Connection;
use zbus::connection::Builder;
use zbus::zvariant::OwnedObjectPath;

use crate::tree::AtspiElement;

pub(crate) const BUS: &str = ":1.7";
pub(crate) const TABLE_PATH: &str = "/table";
pub(crate) const CELL: i32 = 10;

#[derive(Default)]
pub(crate) struct Calls {
    pub(crate) get_children: AtomicUsize,
    pub(crate) child_at_index: AtomicUsize,
    pub(crate) accessible_at: AtomicUsize,
}

pub(crate) struct Grid {
    reported: i32,
    calls: Arc<Calls>,
}

fn cell_path(index: i32) -> OwnedObjectPath {
    OwnedObjectPath::try_from(format!("/cell/{index}")).expect("a cell path parses")
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Grid {
    #[zbus(property)]
    fn child_count(&self) -> i32 {
        self.reported
    }

    fn get_role(&self) -> u32 {
        55
    }

    fn get_children(&self) -> Vec<(String, OwnedObjectPath)> {
        self.calls.get_children.fetch_add(1, Ordering::SeqCst);
        (0..self.reported.min(8))
            .map(|index| (BUS.to_owned(), cell_path(index)))
            .collect()
    }

    fn get_child_at_index(&self, index: i32) -> (String, OwnedObjectPath) {
        self.calls.child_at_index.fetch_add(1, Ordering::SeqCst);
        (BUS.to_owned(), cell_path(index))
    }

    fn get_interfaces(&self) -> Vec<String> {
        vec![
            "org.a11y.atspi.Accessible".to_owned(),
            "org.a11y.atspi.Component".to_owned(),
            "org.a11y.atspi.Table".to_owned(),
        ]
    }
}

pub(crate) struct Component(i32, i32, bool);

#[zbus::interface(name = "org.a11y.atspi.Component")]
impl Component {
    fn get_extents(&self, _coordinates: u32) -> (i32, i32, i32, i32) {
        (0, 0, self.0 * CELL, self.1 * CELL)
    }

    fn get_accessible_at_point(
        &self,
        x: i32,
        y: i32,
        _coordinates: u32,
    ) -> (String, OwnedObjectPath) {
        if self.2 {
            return (
                String::new(),
                OwnedObjectPath::try_from("/org/a11y/atspi/null").expect("the null path parses"),
            );
        }
        (BUS.to_owned(), cell_path((y / CELL) * self.0 + x / CELL))
    }
}

pub(crate) struct Table(i32, Arc<Calls>, bool, Option<i32>);

#[zbus::interface(name = "org.a11y.atspi.Table")]
impl Table {
    fn get_accessible_at(
        &self,
        row: i32,
        column: i32,
    ) -> zbus::fdo::Result<(String, OwnedObjectPath)> {
        self.1.accessible_at.fetch_add(1, Ordering::SeqCst);
        let index = row * self.0 + column;
        if self.3 == Some(index) {
            return Err(zbus::fdo::Error::Failed(
                "the cell cannot be read".to_owned(),
            ));
        }
        if self.2 {
            let path =
                OwnedObjectPath::try_from(format!("/fresh/{index}")).expect("a cell path parses");
            return Ok((BUS.to_owned(), path));
        }
        Ok((BUS.to_owned(), cell_path(index)))
    }

    fn get_index_at(&self, row: i32, column: i32) -> i32 {
        row * self.0 + column
    }

    fn get_row_at_index(&self, index: i32) -> i32 {
        index / self.0
    }

    fn get_column_at_index(&self, index: i32) -> i32 {
        index % self.0
    }
}

pub(crate) struct Cell(i32, bool);

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Cell {
    #[zbus(property)]
    fn parent(&self) -> (String, OwnedObjectPath) {
        let path = if self.1 { "/wrapper" } else { TABLE_PATH };
        (
            BUS.to_owned(),
            OwnedObjectPath::try_from(path).expect("a parent path parses"),
        )
    }

    fn get_index_in_parent(&self) -> i32 {
        self.0
    }
}

/// A table with `columns` columns and `view_rows` rows in view that reports
/// `reported` children, with the cells at the two corners of the view served.
pub(crate) fn grid(
    reported: i32,
    columns: i32,
    view_rows: i32,
) -> (Connection, AtspiElement, Arc<Calls>, Connection) {
    grid_with(reported, columns, view_rows, Quirks::default())
}

/// How the fake table departs from a well-behaved one. `fresh_objects` makes
/// the table answer with a new object path for a cell each time, as a toolkit
/// that creates cell objects on demand does. `blind` makes a point query at
/// the table answer with the null object, as a table that shows no cell there
/// does. `nested_hit` makes the cells at the corners report a wrapper, not the
/// table, as their parent. `failing_cell` makes the lookup of the cell at that
/// index fail.
#[derive(Default, Clone, Copy)]
pub(crate) struct Quirks {
    pub(crate) fresh_objects: bool,
    pub(crate) blind: bool,
    pub(crate) nested_hit: bool,
    pub(crate) failing_cell: Option<i32>,
}

/// The same table with the departures `quirks` names.
pub(crate) fn grid_with(
    reported: i32,
    columns: i32,
    view_rows: i32,
    quirks: Quirks,
) -> (Connection, AtspiElement, Arc<Calls>, Connection) {
    let calls = Arc::new(Calls::default());
    let (near, far) = std::os::unix::net::UnixStream::pair().expect("a socket pair opens");
    let corner = (view_rows - 1) * columns + (columns - 1);
    let builder = Builder::async_io_unix_stream(far)
        .server(zbus::Guid::generate())
        .expect("a server builds")
        .p2p()
        .serve_at(
            TABLE_PATH,
            Grid {
                reported,
                calls: calls.clone(),
            },
        )
        .and_then(|builder| {
            builder.serve_at(TABLE_PATH, Component(columns, view_rows, quirks.blind))
        })
        .and_then(|builder| {
            builder.serve_at(
                TABLE_PATH,
                Table(
                    columns,
                    calls.clone(),
                    quirks.fresh_objects,
                    quirks.failing_cell,
                ),
            )
        })
        .and_then(|builder| builder.serve_at("/cell/0", Cell(0, quirks.nested_hit)))
        .and_then(|builder| {
            builder.serve_at(format!("/cell/{corner}"), Cell(corner, quirks.nested_hit))
        })
        .expect("the objects are served");
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        builder.build(),
    ));
    let element = AtspiElement::new(BUS, TABLE_PATH, 1);
    (
        client.expect("the client connects"),
        element,
        calls,
        server.expect("the server starts"),
    )
}
