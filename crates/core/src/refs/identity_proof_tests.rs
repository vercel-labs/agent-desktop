use crate::{IdentityProof, RefEntryIdentity};

fn stored(field: &str) -> IdentityProof {
    let json = format!(r#"{{"role":"button","name":null,{field}}}"#);
    serde_json::from_str::<RefEntryIdentity>(&json)
        .expect("the identity deserializes")
        .proof
}

#[test]
fn an_earlier_release_bool_maps_to_unique_or_unproven() {
    assert_eq!(stored(r#""unique_at_capture":true"#), IdentityProof::Unique);
    assert_eq!(
        stored(r#""unique_at_capture":false"#),
        IdentityProof::Unproven
    );
}

#[test]
fn a_stored_proof_and_an_absent_one_read_back() {
    assert_eq!(stored(r#""proof":"twins""#), IdentityProof::Twins);
    assert_eq!(stored(r#""proof":"unique""#), IdentityProof::Unique);
    assert_eq!(stored(r#""x":1"#), IdentityProof::Unproven);
}
