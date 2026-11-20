use crate::refs::ref_token::{qualify_ref_id, resolve_ref_target, validate_ref_token};

#[test]
fn qualified_ref_selects_its_own_snapshot() {
    assert_eq!(
        resolve_ref_target("@sabc:e7").unwrap(),
        ("sabc".into(), "@e7".into())
    );
    assert_eq!(qualify_ref_id("sabc", "@e7"), "@sabc:e7");
}

#[test]
fn bare_ref_is_rejected_with_a_qualified_ref_suggestion() {
    for bare in ["@e1", "@e14", "@e999"] {
        let error = resolve_ref_target(bare).unwrap_err();
        assert_eq!(error.code(), "INVALID_ARGS");
        assert!(
            error
                .suggestion()
                .is_some_and(|s| s.contains("@s8f3k2p9:e1")),
            "{bare}: suggestion must name the qualified form"
        );
    }
}

#[test]
fn syntax_validation_accepts_only_the_qualified_form() {
    assert!(validate_ref_token("@sabc:e1").is_ok());
    assert!(validate_ref_token("@e1").is_err());
    assert!(validate_ref_token("@latest:e0").is_err());
    assert!(validate_ref_token("@sabc:e").is_err());
    assert!(validate_ref_token("sabc:e1").is_err());
}

#[test]
fn ref_emptied_by_a_zsh_modifier_gets_the_braced_form_suggestion() {
    for emptied in ["@5", "@10", "@123"] {
        let error = resolve_ref_target(emptied).unwrap_err();
        assert_eq!(error.code(), "INVALID_ARGS");
        assert!(
            error
                .suggestion()
                .is_some_and(|s| s.contains("@${SNAP}:e5")),
            "{emptied}: suggestion must name the braced shell form"
        );
    }
    let bare = resolve_ref_target("@e5").unwrap_err();
    assert!(
        bare.suggestion()
            .is_some_and(|s| !s.contains("@${SNAP}:e5")),
        "a bare @eN ref keeps the qualified-ref suggestion"
    );
}
