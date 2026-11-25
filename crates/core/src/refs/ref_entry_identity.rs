use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RefEntryIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_object: Option<String>,
    pub role: String,
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<crate::ElementIdentifier>,
    /// What the capture established: `Unique` when it read complete evidence and
    /// exactly one observed element matched this entry's full identity predicate.
    #[serde(
        default,
        alias = "unique_at_capture",
        deserialize_with = "crate::refs::identity_proof::deserialize_proof_or_legacy_bool",
        skip_serializing_if = "crate::IdentityProof::is_unproven"
    )]
    pub proof: crate::IdentityProof,
}
