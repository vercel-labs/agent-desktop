use super::front_window::FrontWindow;

/// What the user sees in front: the window that has the focus, the topmost
/// window, and every window open at the moment of the read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrontView {
    pub focused: Option<FrontWindow>,
    pub top: Option<FrontWindow>,
    pub windows: Vec<String>,
    /// The processes that own the listed windows, when the adapter can say.
    pub owners: Vec<u32>,
}

impl FrontView {
    pub(crate) fn has_window(&self, id: &str) -> bool {
        self.windows.iter().any(|window| window == id)
    }
}
