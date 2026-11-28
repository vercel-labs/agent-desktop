/// What reading one node on the way down a stored path yields: its children
/// in the order the walk indexes them, and what the read could not settle.
/// `positions` is set when the children are only some of the node's: it holds
/// the index in the parent of each listed child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Level<K> {
    pub(crate) children: Vec<K>,
    pub(crate) children_unread: bool,
    pub(crate) empty_means_unread: bool,
    pub(crate) positions: Option<Vec<usize>>,
}

impl<K> Level<K> {
    /// The child the walk records at `index` of its parent.
    pub(crate) fn child_at(self, index: usize) -> Option<K> {
        match self.positions {
            Some(positions) => {
                let place = positions.iter().position(|listed| *listed == index)?;
                self.children.into_iter().nth(place)
            }
            None => self.children.into_iter().nth(index),
        }
    }
}

#[cfg(test)]
#[path = "level_tests.rs"]
mod tests;
