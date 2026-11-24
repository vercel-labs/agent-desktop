use super::*;

#[test]
fn sha256_matches_standard_test_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn canonical_hash_distinguishes_all_structural_and_content_properties() {
    let base = AccessibilityNode {
        ref_id: Some("@s1:e1".into()),
        role: "button".into(),
        identity: crate::NodeIdentity {
            name: Some("Save".into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: vec![],
    };

    let same_diff_snap = AccessibilityNode {
        ref_id: Some("@s99:e1".into()),
        ..base.clone()
    };
    assert_eq!(
        hash_canonical_tree(&base),
        hash_canonical_tree(&same_diff_snap)
    );

    let mut diff_state = base.clone();
    diff_state.presentation.states = vec!["focused".into()];
    assert_ne!(hash_canonical_tree(&base), hash_canonical_tree(&diff_state));

    let mut diff_bounds = base.clone();
    diff_bounds.presentation.bounds = Some(Rect {
        x: 1.0,
        y: 2.0,
        width: 30.0,
        height: 40.0,
    });
    assert_ne!(
        hash_canonical_tree(&base),
        hash_canonical_tree(&diff_bounds)
    );

    let mut neg_zero = diff_bounds.clone();
    neg_zero.presentation.bounds = Some(Rect {
        x: 1.0,
        y: 2.0,
        width: -0.0,
        height: 40.0,
    });
    let mut pos_zero = diff_bounds.clone();
    pos_zero.presentation.bounds = Some(Rect {
        x: 1.0,
        y: 2.0,
        width: 0.0,
        height: 40.0,
    });
    assert_eq!(
        hash_canonical_tree(&neg_zero),
        hash_canonical_tree(&pos_zero)
    );
}
