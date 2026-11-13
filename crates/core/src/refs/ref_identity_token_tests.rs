use super::*;

fn stored(token: &str) -> RefEntry {
    let mut saved = entry();
    saved.process.process_instance = Some("macos-proc-v1:10:20".into());
    saved.identity.retained_object = Some(token.into());
    saved
}

fn compare(saved: &RefEntry, actual: &str) -> IdentityMatch {
    identity_match(
        saved,
        &LocatorField::Absent,
        &LocatorField::Absent,
        &LocatorField::Absent,
        &IdentifierEvidence::absent().with_retained_object(actual.into()),
    )
}

#[test]
fn shipped_ref_matches_owned_live_token_for_the_same_host_and_object() {
    let saved = stored("axtok1:abcd");
    assert_eq!(
        compare(&saved, "axtok2:1:macos-proc-v1:10:20:abcd"),
        IdentityMatch::Match
    );
    assert_eq!(compare(&saved, "axtok1:abcd"), IdentityMatch::Match);
}

#[test]
fn shipped_ref_rejects_another_owner_generation_or_object() {
    let saved = stored("axtok1:abcd");
    for actual in [
        "axtok2:2:macos-proc-v1:10:20:abcd",
        "axtok2:1:macos-proc-v1:11:20:abcd",
        "axtok2:1:macos-proc-v1:10:21:abcd",
        "axtok2:1:macos-proc-v1:10:20:ffff",
        "axtok1:ffff",
        "path:abcd",
    ] {
        assert_eq!(compare(&saved, actual), IdentityMatch::NoMatch, "{actual}");
    }
}

#[test]
fn owned_refs_keep_all_owner_evidence_when_compared() {
    let saved = stored("axtok2:1:macos-proc-v1:10:20:abcd");
    assert_eq!(
        compare(&saved, "axtok2:1:macos-proc-v1:10:20:abcd"),
        IdentityMatch::Match
    );
    for actual in [
        "axtok1:abcd",
        "axtok2:1:macos-proc-v1:11:20:abcd",
        "axtok2:2:macos-proc-v1:10:20:abcd",
    ] {
        assert_eq!(compare(&saved, actual), IdentityMatch::NoMatch, "{actual}");
    }
}

#[test]
fn shipped_ref_without_a_host_instance_cannot_match_an_owned_token() {
    for instance in [None, Some(String::new())] {
        let mut saved = stored("axtok1:abcd");
        saved.process.process_instance = instance;
        assert_eq!(
            compare(&saved, "axtok2:1:macos-proc-v1:10:20:abcd"),
            IdentityMatch::Unknown
        );
    }
}
