use crate::{
    AdapterError, ErrorCode, LiveIdentity, LocatorEvidence, LocatorField, Rect,
    tree::element_state::ElementState,
};

#[derive(Debug, Clone)]
pub struct LiveElement {
    pub identity: LiveIdentity,
    pub state: ElementState,
    pub states_complete: bool,
    pub bounds: Option<Rect>,
    pub available_actions: Vec<String>,
}

/// The answer for a live read that left a field the projection needs unread.
/// It is retryable: the node answered, a field did not.
pub fn incomplete_live_evidence() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Live element evidence was incomplete",
    )
    .with_details(serde_json::json!({
        "kind": "live_element_evidence",
        "complete": false,
        "retryable": true,
    }))
}

/// The answer for a value read of text longer than the adapter's read limit.
/// The app answered, so waiting never helps: `get` names the length and the
/// limit, and commands that do not read the value are unaffected.
pub fn value_exceeds_read_limit(length: usize, limit: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The value is {length} characters, longer than the {limit} character read limit"),
    )
    .with_suggestion(
        "The field answered; only reading its text is refused. Other commands on this ref still work.",
    )
    .with_details(serde_json::json!({
        "kind": "value_exceeds_read_limit",
        "value_length": length,
        "read_limit": limit,
        "retryable": false,
    }))
}

/// Whether every slot a single-element live read must answer was observed:
/// role, value, states, bounds and available actions.
pub fn essential_live_evidence_complete(evidence: &LocatorEvidence) -> bool {
    essential_live_evidence_complete_with(evidence, true)
}

/// The same check for a read whose value may be settled another way: an app
/// that answered with text past the read limit has a value that is known to
/// exist and cannot be carried, so a command that never reads it must not fail
/// on it. A value is never identity.
pub fn essential_live_evidence_complete_with(
    evidence: &LocatorEvidence,
    value_required: bool,
) -> bool {
    !evidence.role.is_unknown()
        && (!value_required || !evidence.value.is_unknown())
        && !evidence.states.is_unknown()
        && !evidence.ref_evidence.bounds.is_unknown()
        && !evidence.ref_evidence.available_actions.is_unknown()
}

/// The available actions of a live read, with an absent list read as empty
/// and an unread one refused as incomplete evidence.
pub fn known_actions(actions: LocatorField<Vec<String>>) -> Result<Vec<String>, AdapterError> {
    match actions {
        LocatorField::Known(actions) => Ok(actions),
        LocatorField::Absent => Ok(Vec::new()),
        LocatorField::Unknown => Err(incomplete_live_evidence()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdentifierEvidence, LocatorRefEvidence};

    fn evidence(role: &str, name: Option<&str>) -> LocatorEvidence {
        LocatorEvidence {
            role: LocatorField::Known(role.to_string()),
            name: name
                .map(|value| LocatorField::Known(value.to_string()))
                .unwrap_or(LocatorField::Absent),
            description: LocatorField::Absent,
            value: LocatorField::Absent,
            identifiers: IdentifierEvidence::absent(),
            states: LocatorField::Known(Vec::new()),
            ref_evidence: LocatorRefEvidence {
                window_bounds: LocatorField::Absent,
                bounds: LocatorField::Absent,
                available_actions: LocatorField::Known(Vec::new()),
                descriptors: Default::default(),
            },
        }
    }

    #[test]
    fn incomplete_evidence_is_the_retryable_unresponsive_answer() {
        let error = incomplete_live_evidence();

        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(error.message, "Live element evidence was incomplete");
        assert_eq!(
            error.details,
            Some(serde_json::json!({
                "kind": "live_element_evidence",
                "complete": false,
                "retryable": true,
            }))
        );
    }

    #[test]
    fn essential_evidence_needs_every_essential_slot_observed() {
        assert!(essential_live_evidence_complete(&evidence("button", None)));

        let unread: [fn(&mut LocatorEvidence); 5] = [
            |evidence| evidence.role = LocatorField::Unknown,
            |evidence| evidence.value = LocatorField::Unknown,
            |evidence| evidence.states = LocatorField::Unknown,
            |evidence| evidence.ref_evidence.bounds = LocatorField::Unknown,
            |evidence| evidence.ref_evidence.available_actions = LocatorField::Unknown,
        ];
        for leave_unread in unread {
            let mut partial = evidence("button", None);
            leave_unread(&mut partial);
            assert!(!essential_live_evidence_complete(&partial));
        }
    }

    #[test]
    fn known_actions_reads_absent_as_empty_and_refuses_unknown() {
        let actions = vec!["Press".to_string()];

        assert_eq!(
            known_actions(LocatorField::Known(actions.clone())).ok(),
            Some(actions)
        );
        assert_eq!(known_actions(LocatorField::Absent).ok(), Some(Vec::new()));
        let refused = known_actions(LocatorField::Unknown).err();
        assert_eq!(
            refused.map(|error| error.code),
            Some(ErrorCode::AppUnresponsive)
        );
    }

    #[test]
    fn a_value_past_the_read_limit_is_a_final_answer_not_a_retryable_one() {
        let error = value_exceeds_read_limit(5000, 2048);

        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert!(!error.is_explicitly_retryable());
        let details = error.details.expect("details");
        assert_eq!(details["kind"], "value_exceeds_read_limit");
        assert_eq!(details["value_length"], 5000);
        assert_eq!(details["read_limit"], 2048);
    }

    #[test]
    fn a_value_that_cannot_be_carried_excuses_only_the_value_slot() {
        let mut unread = evidence("textfield", None);
        unread.value = LocatorField::Unknown;
        unread.ref_evidence.bounds = LocatorField::Known(Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });

        assert!(!essential_live_evidence_complete(&unread));
        assert!(essential_live_evidence_complete_with(&unread, false));
        unread.role = LocatorField::Unknown;
        assert!(!essential_live_evidence_complete_with(&unread, false));
    }
}
