/// Who had the focus before an operation that can take it.
#[derive(Clone, Copy)]
pub(crate) enum PriorFocus<P> {
    Held(P),
    Nobody,
    Unreadable,
}

impl<P> PriorFocus<P> {
    pub(crate) fn held(self) -> Option<P> {
        match self {
            Self::Held(prior) => Some(prior),
            Self::Nobody | Self::Unreadable => None,
        }
    }
}
