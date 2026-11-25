use serde::{Deserialize, Serialize};

/// What the capture that recorded a ref established about the ref's identity.
///
/// Only `Unique` lets a tokenless ref resolve. The other two refuse for
/// different reasons, so an error can name the one that applies: `Twins` means
/// a complete read found another element with the same recorded identity, and no
/// later snapshot can separate them; `Unproven` means the capture did not read
/// the whole window, or a node left a recorded field unread, so a snapshot with
/// more budget can still prove the ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityProof {
    #[default]
    Unproven,
    Unique,
    Twins,
}

impl IdentityProof {
    pub fn is_unique(self) -> bool {
        self == Self::Unique
    }

    pub(crate) fn is_unproven(&self) -> bool {
        *self == Self::Unproven
    }
}
