use serde_json::{Value, json};

use super::front_guard_state::FrontGuardState;

/// What a front guard did during one headless command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontOutcome {
    Off,
    Clean,
    GivenBack,
    GiveBackFailed,
}

pub(crate) const RETURN_FAILED_SUGGESTION: &str = "The application took the focus or the top of the window order and could not be sent back. Run 'focus-window' on the window that had the focus.";

impl FrontOutcome {
    pub(crate) fn from_state(state: FrontGuardState) -> Self {
        match state {
            FrontGuardState::GivenBack => Self::GivenBack,
            FrontGuardState::GiveBackFailed => Self::GiveBackFailed,
            FrontGuardState::Clean | FrontGuardState::Watching | FrontGuardState::Intruded => {
                Self::Clean
            }
        }
    }

    /// True only when a read after the give-back showed the user's front again.
    pub fn focus_returned(self) -> bool {
        self == Self::GivenBack
    }

    /// The same record on an error a guarded command ended with.
    pub(crate) fn note_in_error(self, error: &mut crate::AdapterError) {
        self.note_in(&mut error.details);
    }

    /// Records an intrusion in a result's `details`, under the same
    /// `focus_returned` name a launch reports it by. A clean run adds nothing.
    pub(crate) fn note_in(self, details: &mut Option<Value>) {
        let returned = match self {
            Self::GivenBack => true,
            Self::GiveBackFailed => false,
            Self::Off | Self::Clean => return,
        };
        let object = details.get_or_insert_with(|| json!({}));
        if let Some(object) = object.as_object_mut() {
            object.insert("focus_returned".into(), json!(returned));
            if !returned {
                object
                    .entry("suggestion")
                    .or_insert_with(|| json!(RETURN_FAILED_SUGGESTION));
            }
        }
    }
}
