use agent_desktop_core::SurfaceInfo;

/// One window as the inventory read it, before the surfaces it presents are
/// turned into [`SurfaceInfo`] ids. Keeping the two steps apart lets the id
/// rules be exercised without a desktop.
pub(crate) struct WindowObservation {
    pub(crate) number: Option<i64>,
    pub(crate) kind: Option<&'static str>,
    pub(crate) title: Option<String>,
    pub(crate) children: Vec<ChildSurface>,
    pub(crate) popover: ObservedPopover,
}

/// A direct child of a window that is itself a surface.
pub(crate) struct ChildSurface {
    pub(crate) number: Option<i64>,
    pub(crate) kind: &'static str,
    pub(crate) title: Option<String>,
    pub(crate) index: usize,
}

/// What the bounded popover search of one window settled on.
pub(crate) enum ObservedPopover {
    /// The search read every element and found no popover.
    Absent,
    /// The search was cut at its element limit or left a container closed, so
    /// a popover may be there and was not read.
    Unread { title: Option<String> },
    Found {
        number: Option<i64>,
        title: Option<String>,
    },
}

/// The surfaces one window presents, identified by stable AX window numbers.
///
/// A window's id is its own number, the one `list-windows` reports as
/// `w-<number>`. An unlisted popover uses its own number too, and a surface
/// presented inside a window that has no number of its own uses its parent's
/// stable id plus its kind and position among that parent's children. Closing
/// an earlier window then never re-ids a surface that survives it, which a
/// signal diff would otherwise read as one dismissal and one appearance.
///
/// Only what cannot be identified is reported unread: a window whose own
/// number is missing still lists a surface that carries a number of its own,
/// and a plain window it presents nothing on adds no entry at all.
pub(crate) fn window_surfaces(window: WindowObservation) -> Vec<SurfaceInfo> {
    let window_id = window.number.filter(|number| *number > 0).map(surface_id);
    let mut surfaces = Vec::new();
    let mut unread = Vec::new();
    if let Some(kind) = window.kind {
        match window_id.as_deref() {
            Some(id) => surfaces.push(surface(id.to_owned(), kind, window.title.clone())),
            None => push_unread_kind(&mut unread, kind),
        }
    }
    for child in window.children {
        match (
            child.number.filter(|number| *number > 0),
            window_id.as_deref(),
        ) {
            (Some(number), _) => {
                surfaces.push(surface(surface_id(number), child.kind, child.title))
            }
            (None, Some(id)) => surfaces.push(surface(
                format!("{id}/{}/{}", child.kind, child.index),
                child.kind,
                child.title,
            )),
            (None, None) => push_unread_kind(&mut unread, child.kind),
        }
    }
    match (window.popover, window_id.as_deref()) {
        (ObservedPopover::Absent, _) => {}
        (ObservedPopover::Unread { title }, Some(id)) => {
            mark_popover_unread(&mut surfaces, id, title)
        }
        (ObservedPopover::Unread { .. }, None) => push_unread_kind(&mut unread, "popover"),
        (ObservedPopover::Found { number, title }, parent) => {
            match (number.filter(|number| *number > 0), parent) {
                (Some(number), _) => surfaces.push(surface(surface_id(number), "popover", title)),
                (None, Some(id)) => {
                    surfaces.push(surface(format!("{id}/popover"), "popover", title))
                }
                (None, None) => push_unread_kind(&mut unread, "popover"),
            }
        }
    }
    if !unread.is_empty() {
        surfaces.push(unread_window(unread));
    }
    surfaces
}

fn push_unread_kind(unread: &mut Vec<&'static str>, kind: &'static str) {
    if !unread.contains(&kind) {
        unread.push(kind);
    }
}

/// A window whose own AX window number cannot be read, listing the kinds it
/// could present that were not identified. Marking them unread keeps a missing
/// entry from reading as an observed absence.
fn unread_window(kinds: Vec<&'static str>) -> SurfaceInfo {
    SurfaceInfo {
        id: "window/unread".into(),
        kind: "window".into(),
        title: None,
        item_count: None,
        unclassified: kinds.into_iter().map(str::to_string).collect(),
    }
}

pub(crate) fn surface(id: String, kind: &str, title: Option<String>) -> SurfaceInfo {
    SurfaceInfo {
        id,
        kind: kind.into(),
        title,
        item_count: None,
        unclassified: Vec::new(),
    }
}

/// Adds `surface` unless the inventory already lists that id as that kind: one
/// window can be reached both as a window and as a surface of its own, and a
/// surface read twice is still one surface.
pub(crate) fn push_unique(surfaces: &mut Vec<SurfaceInfo>, surface: SurfaceInfo) {
    let listed = surfaces
        .iter()
        .any(|listed| listed.id == surface.id && listed.kind == surface.kind);
    if !listed {
        surfaces.push(surface);
    }
}

/// The id `list-windows` and `snapshot --window-id` use for the same window.
pub(crate) fn surface_id(number: i64) -> String {
    format!("w-{number}")
}

fn mark_popover_unread(surfaces: &mut Vec<SurfaceInfo>, window_id: &str, title: Option<String>) {
    match surfaces.iter_mut().find(|entry| entry.id == window_id) {
        Some(entry) => entry.unclassified.push("popover".into()),
        None => {
            let mut entry = surface(window_id.to_owned(), "window", title);
            entry.unclassified.push("popover".into());
            surfaces.push(entry);
        }
    }
}

#[cfg(test)]
#[path = "surface_ids_tests.rs"]
mod tests;
