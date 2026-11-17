//! Reads all of a node's children in one cross-process call.
//!
//! UI Automation cannot cap how many children a cached read returns, so a
//! batch is only ever asked of a container that is not a collection. Lists,
//! grids, tables, trees and documents are where a single node holds thousands
//! of items, and those stay on the one-by-one walk, which stops at the
//! sibling cap and the deadline.

use crate::tree::element::UIAElement;
use crate::tree::properties::ElementProperties;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::properties::property_outcome::{PropertyOutcome, PropertyValue};
use agent_desktop_core::Deadline;
use uiautomation::core::UICacheRequest;

/// UI Automation answers `GetCachedChildren` for a node without children with
/// success and a null array, which the binding reports as an error whose code
/// is `S_OK`. Any other failure leaves the node to the live walk.
const NO_CACHED_CHILDREN: i32 = 0;

/// `ComboBox`, `ListItem`, `List`, `Tree`, `TreeItem`, `DataGrid`, `DataItem`,
/// `Document` and `Table`: the control types that hold an application's items.
const COLLECTION_CONTROL_TYPES: [i32; 9] = [
    50003, 50007, 50008, 50023, 50024, 50028, 50029, 50030, 50036,
];

pub(crate) struct ChildBatch {
    request: UICacheRequest,
}

impl ChildBatch {
    pub(crate) fn new(request: UICacheRequest) -> Self {
        Self { request }
    }

    /// The node's children in order, or `None` when the node is a collection,
    /// the deadline has passed, or the read fails, all of which leave it to
    /// the one-by-one walk.
    pub(crate) fn read(
        &self,
        node: &UIAElement,
        properties: &ElementProperties,
        deadline: Deadline,
    ) -> Option<Vec<UIAElement>> {
        if holds_items(properties) || deadline.is_expired() {
            return None;
        }
        let updated = node.0.build_updated_cache(&self.request).ok()?;
        match updated.get_cached_children() {
            Ok(children) => Some(children.into_iter().map(UIAElement::from).collect()),
            Err(error) if error.code() == NO_CACHED_CHILDREN => Some(Vec::new()),
            Err(_) => None,
        }
    }
}

/// Whether the node is a collection, or its control type is unknown. Either
/// way the batch is not used, because nothing then bounds the list.
pub(crate) fn holds_items(properties: &ElementProperties) -> bool {
    match properties.get(TreeProperty::ControlType) {
        PropertyOutcome::Known(PropertyValue::Number(id)) if id != 0 => {
            COLLECTION_CONTROL_TYPES.contains(&id)
        }
        _ => true,
    }
}

#[cfg(test)]
#[path = "child_batch_tests.rs"]
mod tests;
