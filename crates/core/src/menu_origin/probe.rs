//! Reading the menu surface a caller is about to act on.

use crate::adapter::{PlatformAdapter, TreeOptions};
use crate::{Deadline, SnapshotSurface, snapshot};

use super::instance::{self, MenuInstance};

const MENU_DEPTH: u8 = 6;

/// What a bounded read of the menu surface answered.
#[derive(Debug)]
pub(crate) enum MenuRead {
    /// A complete read proved no menu is open.
    Absent,
    /// A complete read observed this menu, and the platform named its element.
    Observed(MenuInstance),
    /// A complete read observed a menu the platform did not name.
    Unidentified,
    /// The read could not answer, so nothing may be concluded either way.
    Unknown,
}

/// Reads the menu surface of `app` within `deadline`.
///
/// A platform that cannot serve the surface answers [`MenuRead::Unknown`]: a
/// menu this driver cannot read is not a menu it can vouch for.
pub(crate) fn observe(adapter: &dyn PlatformAdapter, app: &str, deadline: Deadline) -> MenuRead {
    if adapter
        .check_surface_support(SnapshotSurface::Menu)
        .is_err()
    {
        return MenuRead::Unknown;
    }
    let opts = TreeOptions {
        max_depth: MENU_DEPTH,
        include_bounds: true,
        interactive_only: false,
        compact: false,
        surface: SnapshotSurface::Menu,
        skeleton: false,
        force_renderer_accessibility: false,
    };
    match snapshot::build(adapter, &opts, Some(app), None, deadline) {
        Ok(result) if !result.complete => MenuRead::Unknown,
        Ok(result) => match instance::of_tree(&result.tree) {
            Some(menu) => MenuRead::Observed(menu),
            None => MenuRead::Unidentified,
        },
        Err(crate::AppError::Adapter(error))
            if error.is_confirmed_surface_absence(SnapshotSurface::Menu) =>
        {
            MenuRead::Absent
        }
        Err(_) => MenuRead::Unknown,
    }
}
