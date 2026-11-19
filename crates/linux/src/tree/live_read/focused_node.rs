//! The node that holds keyboard focus, in one process or on the whole
//! desktop. AT-SPI has no call that names it, so it is searched for inside
//! each top-level frame that is `ACTIVE`: through the `Collection` interface
//! where the toolkit implements it (GTK 3, Chromium), and by a walk down the
//! `SHOWING` nodes where it does not (GTK 4).
use std::collections::HashMap;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use zbus::Connection;
use zbus::zvariant::OwnedObjectPath;

use crate::system::registry::{app_query::AppQuery, read_registry_query};
use crate::system::{ACCESSIBLE_INTERFACE, SharedBus, within};
use crate::tree::element::ancestry::Ancestry;
use crate::tree::vocabulary::state_bits::{ACTIVE, FOCUSED, SHOWING, StateBits};
use crate::tree::{AtspiElement, call, list_children};

/// The most nodes a walk reads before it gives up on finding focus.
pub(crate) const MAX_FOCUS_VISITS: usize = 2_048;

const COLLECTION_INTERFACE: &str = "org.a11y.atspi.Collection";
const MATCH_ALL: i32 = 1;
const SORT_CANONICAL: u32 = 1;

/// What the focus search needs from the accessibility tree. A node that
/// cannot be read answers `None` or no children; only a spent deadline is an
/// error.
pub(crate) trait FocusSource {
    type Node;
    fn states(&mut self, node: &Self::Node) -> Result<Option<Vec<u32>>, AdapterError>;
    fn children(&mut self, node: &Self::Node) -> Result<Vec<Self::Node>, AdapterError>;
    /// The focused descendant `Collection` reports, `Some(None)` when it
    /// reports none, and `None` when the frame does not implement it.
    fn collection_focus(
        &mut self,
        frame: &Self::Node,
    ) -> Result<Option<Option<Self::Node>>, AdapterError>;
}

fn has<S: FocusSource>(source: &mut S, node: &S::Node, bit: u32) -> Result<bool, AdapterError> {
    Ok(source
        .states(node)?
        .is_some_and(|words| StateBits::new(&words).has(bit)))
}

/// The focused node inside `frame`, through `Collection` when the frame
/// implements it, else by a walk that skips every subtree not `SHOWING`.
pub(crate) fn focus_in<S: FocusSource>(
    source: &mut S,
    frame: &S::Node,
) -> Result<Option<S::Node>, AdapterError> {
    if let Some(found) = source.collection_focus(frame)? {
        return Ok(found);
    }
    let mut pending = source.children(frame)?;
    pending.reverse();
    let mut visits = 0;
    while let Some(node) = pending.pop() {
        visits += 1;
        if visits > MAX_FOCUS_VISITS {
            return Ok(None);
        }
        let Some(words) = source.states(&node)? else {
            continue;
        };
        let bits = StateBits::new(&words);
        if bits.has(FOCUSED) {
            return Ok(Some(node));
        }
        if bits.has(SHOWING) {
            let mut children = source.children(&node)?;
            children.reverse();
            pending.extend(children);
        }
    }
    Ok(None)
}

/// A frame and the focused node inside it.
pub(crate) type FramedFocus<N> = Option<(N, N)>;

/// The focused node and its frame, searched in the `ACTIVE` frames under
/// `roots` in order.
pub(crate) fn focused_under<S: FocusSource>(
    source: &mut S,
    roots: Vec<S::Node>,
) -> Result<FramedFocus<S::Node>, AdapterError> {
    for root in roots {
        for frame in source.children(&root)? {
            if !has(source, &frame, ACTIVE)? {
                continue;
            }
            if let Some(node) = focus_in(source, &frame)? {
                return Ok(Some((frame, node)));
            }
        }
    }
    Ok(None)
}

struct BusFocus<'a> {
    connection: &'a Connection,
    deadline: Deadline,
}

fn readable<T>(result: Result<Option<T>, AdapterError>) -> Result<Option<T>, AdapterError> {
    match result {
        Err(error) if error.code != ErrorCode::Timeout => Ok(None),
        result => result,
    }
}

impl FocusSource for BusFocus<'_> {
    type Node = AtspiElement;

    fn states(&mut self, node: &AtspiElement) -> Result<Option<Vec<u32>>, AdapterError> {
        readable(within(self.deadline, async {
            Ok(
                call(self.connection, node, ACCESSIBLE_INTERFACE, "GetState", &())
                    .await
                    .and_then(|reply| reply.body().deserialize::<Vec<u32>>())
                    .ok(),
            )
        }))
    }

    fn children(&mut self, node: &AtspiElement) -> Result<Vec<AtspiElement>, AdapterError> {
        let listed = readable(within(self.deadline, async {
            Ok(list_children(self.connection, node).await.ok())
        }))?;
        Ok(listed
            .unwrap_or_default()
            .into_iter()
            .map(|(bus, path)| AtspiElement::new(bus, path, node.pid()))
            .collect())
    }

    fn collection_focus(
        &mut self,
        frame: &AtspiElement,
    ) -> Result<Option<Option<AtspiElement>>, AdapterError> {
        let rule = (
            vec![1_i32 << FOCUSED, 0],
            MATCH_ALL,
            HashMap::<String, String>::new(),
            MATCH_ALL,
            Vec::<i32>::new(),
            MATCH_ALL,
            Vec::<String>::new(),
            MATCH_ALL,
            false,
        );
        let matches = readable(within(self.deadline, async {
            Ok(call(
                self.connection,
                frame,
                COLLECTION_INTERFACE,
                "GetMatches",
                &(rule, SORT_CANONICAL, 1_i32, true),
            )
            .await
            .and_then(|reply| reply.body().deserialize::<Vec<(String, OwnedObjectPath)>>())
            .ok())
        }))?;
        Ok(matches.map(|found| {
            found
                .into_iter()
                .next()
                .map(|(bus, path)| AtspiElement::new(bus, path.to_string(), frame.pid()))
        }))
    }
}

/// The element holding keyboard focus in process `pid`, or anywhere on the
/// desktop outside GNOME Shell when `pid` is `None`, carrying its frame.
/// `None` when no active frame there has a focused node.
pub(crate) fn focused_element(
    bus: &SharedBus,
    pid: Option<u32>,
    deadline: Deadline,
) -> Result<Option<AtspiElement>, AdapterError> {
    let registry = read_registry_query(
        bus,
        &AppQuery {
            pid,
            select: &|_| true,
            enrich: false,
        },
        deadline,
    )?;
    let roots = registry
        .apps
        .into_iter()
        .map(|app| app.root)
        .filter(|root| !crate::system::shell::is_shell_element(root))
        .collect();
    let connection = bus.get(deadline)?;
    let mut source = BusFocus {
        connection: connection.inner(),
        deadline,
    };
    Ok(focused_under(&mut source, roots)?.map(|(frame, mut node)| {
        node.set_ancestry(Ancestry::of(Some(frame), None, false));
        node
    }))
}

#[cfg(test)]
#[path = "focused_node_tests.rs"]
mod tests;
