use serde::{Deserialize, Serialize};

/// What the capture that recorded a ref established about the ref's identity.
///
/// Only `Unique` lets a tokenless ref resolve. The other two refuse for
/// different reasons, so an error can name the one that applies: `Twins` means
/// a complete read found another element with the same recorded identity, and no
/// later snapshot can separate them; `Unproven` means the capture did not read
/// the whole window, or a node left a recorded field unread, so a snapshot with
/// more budget can still prove the ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
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

/// Reads `proof`, or the `unique_at_capture` bool an earlier release stored under
/// the same field: true was a unique capture and false an unproven one, because
/// twins were not a stored state then.
pub(crate) fn deserialize_proof_or_legacy_bool<'de, D>(
    deserializer: D,
) -> Result<IdentityProof, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Legacy(bool),
        Proof(IdentityProof),
    }
    Ok(match Stored::deserialize(deserializer)? {
        Stored::Legacy(true) => IdentityProof::Unique,
        Stored::Legacy(false) => IdentityProof::Unproven,
        Stored::Proof(proof) => proof,
    })
}

#[cfg(test)]
#[path = "identity_proof_tests.rs"]
mod tests;
