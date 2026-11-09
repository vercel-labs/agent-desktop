/// The dialogs a scan of one application's windows found drawn inside them,
/// as titles, and whether every window was read to the end. An incomplete
/// scan still reports what it found: a dialog seen is present, and only a
/// scan that found nothing leaves the presence of a dialog unproven. A blank
/// title is an empty string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::system) struct NestedDialogs {
    pub(in crate::system) titles: Vec<String>,
    pub(in crate::system) complete: bool,
}
