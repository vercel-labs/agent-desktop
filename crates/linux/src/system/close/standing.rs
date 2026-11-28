use agent_desktop_core::{AdapterError, ErrorCode};

use super::refusal;
use super::world::World;
use crate::system::menu::nested_dialogs::NestedDialogs;
use crate::system::x11::control::ControlRead;
use crate::system::x11::window::toplevel::Toplevel;

/// One read of what a process shows that a close request must not answer: its
/// top-level windows, and the titles of the dialogs drawn inside them, which no
/// list of top-level windows holds. `nested` is `None` when that read failed,
/// so the presence of a dialog inside a window is unknown rather than absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Standing {
    pub(super) windows: Vec<Toplevel>,
    pub(super) nested: Option<NestedDialogs>,
}

impl Standing {
    /// A read whose deadline ran out is the command's own timeout, not an
    /// unknown dialog state; every other failure leaves `nested` unknown.
    pub(super) fn of(world: &impl World, windows: Vec<Toplevel>) -> Result<Self, AdapterError> {
        let nested = match world.nested_dialogs() {
            Ok(dialogs) => Some(dialogs),
            Err(error) if error.code == ErrorCode::Timeout => return Err(error),
            Err(_) => None,
        };
        Ok(Self { windows, nested })
    }

    pub(super) fn read(world: &impl World) -> Option<Self> {
        match world.toplevels() {
            ControlRead::Ready(windows) => Self::of(world, windows).ok(),
            _ => None,
        }
    }

    pub(super) fn nested_titles(&self) -> &[String] {
        self.nested
            .as_ref()
            .map(|nested| nested.titles.as_slice())
            .unwrap_or(&[])
    }

    pub(super) fn has_dialog(&self) -> bool {
        !self.nested_titles().is_empty() || self.windows.iter().any(refusal::is_dialog)
    }

    pub(super) fn dialogs_unknown(&self) -> bool {
        self.nested
            .as_ref()
            .is_none_or(|nested| !nested.complete && nested.titles.is_empty())
    }
}
