use super::FrameExtents;
use crate::tree::Extents;

/// One top-level window the window manager manages. `bounds` is the client
/// rectangle in root coordinates, inside the frame `extents` describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct X11Client {
    pub(crate) id: u32,
    pub(crate) pid: Option<u32>,
    pub(crate) title: ClientTitles,
    pub(crate) extents: Option<FrameExtents>,
    pub(crate) bounds: Option<Extents>,
    pub(crate) hidden: bool,
    /// `_GTK_FRAME_EXTENTS` as left, top, right, bottom: the client-side
    /// shadow a GTK window draws inside its client. The toolkit's window
    /// coordinates of GTK 4 start after the left and top; GTK 3 reports the
    /// whole client. Zero for a window with no shadow.
    pub(crate) shadow: (i32, i32, i32, i32),
}

/// The two titles a client can carry. A toolkit's accessibility title may be
/// either one: `_NET_WM_NAME` (UTF-8) or the legacy `WM_NAME` (Latin-1), and
/// the two can differ. Matching a frame accepts a title equal to either.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ClientTitles {
    wide: String,
    narrow: String,
}

impl ClientTitles {
    pub(crate) fn new(wide: String, narrow: String) -> Self {
        Self { wide, narrow }
    }

    /// The title a window list shows: `_NET_WM_NAME` when present, else `WM_NAME`.
    pub(crate) fn display(&self) -> &str {
        if self.wide.is_empty() {
            &self.narrow
        } else {
            &self.wide
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.wide.is_empty() && self.narrow.is_empty()
    }

    pub(crate) fn matches(&self, title: &str) -> bool {
        if title.is_empty() {
            return self.is_empty();
        }
        self.wide == title || self.narrow == title
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        self.matches(other.display()) || other.matches(self.display())
    }
}

impl From<&str> for ClientTitles {
    fn from(title: &str) -> Self {
        Self::new(title.to_owned(), String::new())
    }
}

impl From<String> for ClientTitles {
    fn from(title: String) -> Self {
        Self::new(title, String::new())
    }
}
