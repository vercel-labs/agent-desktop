use crate::tree::element::child_view::ChildSlots;

/// What reading one node of a level produced: its payload and the children to
/// descend into. `children_unread` is set when the node's child list could not
/// be read, so its subtree is unknown; `gone` is set when the node vanished
/// from the bus after its parent listed it. `slots` is set when the node
/// reports more children than were listed: it names the index in the parent of
/// each listed child and the count the node reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expansion<P, K> {
    pub(crate) payload: P,
    pub(crate) children: Vec<K>,
    pub(crate) children_unread: bool,
    pub(crate) gone: bool,
    pub(crate) slots: Option<ChildSlots>,
}

impl<P, K> Expansion<P, K> {
    #[cfg(test)]
    pub(crate) fn read(payload: P, children: Vec<K>) -> Self {
        Self {
            payload,
            children,
            children_unread: false,
            gone: false,
            slots: None,
        }
    }
}
