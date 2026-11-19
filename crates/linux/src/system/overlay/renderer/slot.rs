//! The windows of one cursor, bottom to top in the extension's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Slot {
    Caret,
    Highlight,
    Ripple,
    Pill,
    Glyph,
}

impl Slot {
    pub(crate) const ALL: [Self; 5] = [
        Self::Caret,
        Self::Highlight,
        Self::Ripple,
        Self::Pill,
        Self::Glyph,
    ];
}
