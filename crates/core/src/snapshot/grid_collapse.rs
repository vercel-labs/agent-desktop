//! A table or grid in a skeleton is one node that carries its size. Its cells
//! stay out until a drill asks for them, so an overview of a window does not
//! grow with the tables in it.

use crate::AccessibilityNode;

/// Cuts every table or grid below `root` down to a boundary: its children go
/// and it carries their count, so it anchors a drill like a container the
/// skeleton depth cut. The root keeps its children, because a drill that
/// starts at a grid asks for its cells.
pub(crate) fn collapse_grids(root: &mut AccessibilityNode) {
    for child in &mut root.children {
        if reports_grid_size(child) && !child.children.is_empty() {
            child.children_count = child
                .children_count
                .or(u32::try_from(child.children.len()).ok());
            child.children.clear();
        } else {
            collapse_grids(child);
        }
    }
}

fn reports_grid_size(node: &AccessibilityNode) -> bool {
    let size = &node.presentation.descriptors;
    size.row_count.is_some() || size.column_count.is_some()
}

#[cfg(test)]
#[path = "grid_collapse_tests.rs"]
mod tests;
