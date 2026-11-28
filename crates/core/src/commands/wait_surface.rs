use crate::{AppError, SnapshotSurface, SurfaceInfo};

/// Which surface condition a `wait` targets: `--menu`, `--menu-closed`,
/// `--notification`, or `--surface <kind>`. One variant per flag makes the
/// modes structurally mutually exclusive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceWait {
    Menu,
    MenuClosed,
    Notification,
    /// A surface of this kind is open; `None` is any kind. Unlike an event
    /// wait it is a state, so a surface that opened before the wait counts.
    Open(Option<SnapshotSurface>),
}

impl SurfaceWait {
    pub fn from_flags(
        menu: bool,
        menu_closed: bool,
        notification: bool,
        open: Option<&str>,
    ) -> Result<Option<Self>, AppError> {
        let open = open.map(open_kind).transpose()?.map(Self::Open);
        let mut chosen = [
            menu.then_some(Self::Menu),
            menu_closed.then_some(Self::MenuClosed),
            notification.then_some(Self::Notification),
            open,
        ]
        .into_iter()
        .flatten();
        match (chosen.next(), chosen.next()) {
            (wait, None) => Ok(wait),
            (Some(_), Some(_)) | (None, Some(_)) => {
                Err(crate::commands::wait_mode::ambiguous_wait_mode())
            }
        }
    }
}

fn open_kind(kind: &str) -> Result<Option<SnapshotSurface>, AppError> {
    match kind {
        "any" => Ok(None),
        "popover" => Ok(Some(SnapshotSurface::Popover)),
        "sheet" => Ok(Some(SnapshotSurface::Sheet)),
        "alert" => Ok(Some(SnapshotSurface::Alert)),
        "menu" => Ok(Some(SnapshotSurface::Menu)),
        _ => Err(AppError::invalid_input_with_suggestion(
            format!("Unknown surface kind '{kind}' for wait --surface"),
            "Use popover, sheet, alert, menu or any.",
        )),
    }
}

/// The kinds `wait --surface` names; `any` is any one of them, never a plain
/// window, which Linux and Windows list for every window.
const OPEN_KINDS: [SnapshotSurface; 4] = [
    SnapshotSurface::Popover,
    SnapshotSurface::Sheet,
    SnapshotSurface::Alert,
    SnapshotSurface::Menu,
];

/// Whether a listed surface is of the kind a wait asks for. A context menu is
/// a menu.
pub(crate) fn is_kind(surface: &SurfaceInfo, kind: Option<SnapshotSurface>) -> bool {
    kind_matches(&surface.kind, kind)
}

/// Whether an inventory entry that names `listed` satisfies the wait. Shared by
/// the listed surfaces and the kinds an entry reports as unclassified, so a
/// surface that was not read is judged by the same rule as one that was.
fn kind_matches(listed: &str, kind: Option<SnapshotSurface>) -> bool {
    match kind {
        None => OPEN_KINDS
            .into_iter()
            .any(|kind| kind_matches(listed, Some(kind))),
        Some(SnapshotSurface::Menu) => matches!(listed, "menu" | "context_menu"),
        Some(kind) => listed == kind.as_str(),
    }
}

/// Whether the last observation reports the kind a wait asks for as
/// unclassified: the inventory did not read it, so no matching surface has not
/// shown that the application presented none.
pub(crate) fn unread(surfaces: &[SurfaceInfo], kind: Option<SnapshotSurface>) -> bool {
    surfaces.iter().any(|surface| {
        surface
            .unclassified
            .iter()
            .any(|listed| kind_matches(listed, kind))
    })
}

#[cfg(test)]
#[path = "wait_surface_tests.rs"]
mod tests;
