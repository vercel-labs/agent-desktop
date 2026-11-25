use super::super::identity_fields_are_meaningful;

#[test]
fn identity_fields_report_own_evidence_and_ignore_a_retained_object() {
    assert!(!identity_fields_are_meaningful(
        "button", None, None, None, None
    ));
    assert!(!identity_fields_are_meaningful(
        "button",
        Some(""),
        None,
        Some(""),
        Some("")
    ));
    assert!(identity_fields_are_meaningful(
        "button",
        Some("Save"),
        None,
        None,
        None
    ));
    assert!(identity_fields_are_meaningful(
        "button",
        None,
        None,
        None,
        Some("save")
    ));
    assert!(identity_fields_are_meaningful(
        "textfield",
        Some("Search"),
        None,
        None,
        None
    ));
}

mod unread_value {
    use super::super::super::{IdentityMatch, stable_name_field};
    use crate::locator::live_locator::{
        IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence,
    };
    use crate::refs::ref_full_predicate::FullPredicate;
    use crate::refs::ref_live_predicate::LivePredicate;

    fn live(name: &str, value: LocatorField<String>) -> LocatorEvidence {
        LocatorEvidence {
            role: LocatorField::Known("textfield".into()),
            name: LocatorField::Known(name.into()),
            description: LocatorField::Absent,
            value,
            identifiers: IdentifierEvidence::typed(Vec::new(), None, true),
            states: LocatorField::Known(Vec::new()),
            ref_evidence: LocatorRefEvidence {
                window_bounds: LocatorField::Absent,
                bounds: LocatorField::Absent,
                available_actions: LocatorField::Known(Vec::new()),
                descriptors: Default::default(),
            },
        }
    }

    fn compare(entry_value: Option<&str>, live: &LocatorEvidence) -> IdentityMatch {
        let mut entry = super::super::entry();
        entry.identity.role = "textfield".into();
        entry.identity.name = Some("Text Editor".into());
        entry.identity.value = entry_value.map(str::to_string);
        FullPredicate::of_entry(&entry).compare(&LivePredicate::observed(live))
    }

    #[test]
    fn an_unread_value_leaves_the_recorded_name_as_identity() {
        let name = LocatorField::Known("Text Editor".to_string());
        let unread = LocatorField::<String>::Unknown;
        assert_eq!(
            stable_name_field("textfield", &name, &unread),
            LocatorField::Known("Text Editor")
        );
    }

    #[test]
    fn an_edited_oversize_value_still_resolves_to_the_same_field() {
        let unread = live("Text Editor", LocatorField::Unknown);
        let edited = live("Text Editor", LocatorField::Known("edited".into()));
        assert_eq!(compare(None, &unread), IdentityMatch::Match);
        assert_eq!(compare(None, &edited), IdentityMatch::Match);
    }

    #[test]
    fn a_name_that_repeats_the_value_is_not_identity_after_an_edit() {
        let mut entry = super::super::entry();
        entry.identity.role = "textfield".into();
        entry.identity.name = Some("old text".into());
        entry.identity.value = Some("old text".into());
        let predicate = FullPredicate::of_entry(&entry);
        let edited = live("new text", LocatorField::Known("new text".into()));
        assert_eq!(predicate.mask() & crate::refs::ref_full_predicate::NAME, 0);
        assert_eq!(
            predicate.compare(&LivePredicate::observed(&edited)),
            IdentityMatch::Match
        );
    }

    #[test]
    fn an_empty_value_keeps_the_name() {
        let empty = live("Text Editor", LocatorField::Known(String::new()));
        assert_eq!(compare(None, &empty), IdentityMatch::Match);
    }

    #[test]
    fn a_real_label_stays_identity_while_the_text_changes() {
        let mut entry = super::super::entry();
        entry.identity.role = "textfield".into();
        entry.identity.name = Some("Subject".into());
        entry.identity.value = Some("hello".into());
        let predicate = FullPredicate::of_entry(&entry);
        let edited = live("Subject", LocatorField::Known("changed".into()));
        let other = live("Body", LocatorField::Known("hello".into()));
        assert_eq!(
            predicate.compare(&LivePredicate::observed(&edited)),
            IdentityMatch::Match
        );
        assert_eq!(
            predicate.compare(&LivePredicate::observed(&other)),
            IdentityMatch::NoMatch
        );
    }
}
