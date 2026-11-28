use super::*;
use crate::tree::walk::walker_fake::ref_entry;
use agent_desktop_core::{
    ErrorCode, IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence,
    NodeDescriptor, RefEntry,
};

fn locator_entry(role: &str) -> RefEntry {
    let mut entry = ref_entry(role);
    entry.scope.path_is_absolute = true;
    entry
}

fn locator_evidence(role: &str) -> LocatorEvidence {
    LocatorEvidence {
        role: LocatorField::Known(role.to_string()),
        name: LocatorField::Absent,
        value: LocatorField::Absent,
        description: LocatorField::Absent,
        identifiers: IdentifierEvidence::absent(),
        states: LocatorField::Absent,
        ref_evidence: LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Absent,
            available_actions: LocatorField::Absent,
            descriptors: NodeDescriptor::default(),
        },
    }
}

/// Pins the fix for a tri-state defect where a failed role read was treated
/// the same as a settled role mismatch. A role read that comes back
/// `Unknown` must stay retryable, never a settled `STALE_REF`.
#[test]
fn a_role_read_that_comes_back_unknown_is_retryable_not_settled() {
    let entry = locator_entry("button");
    let mut unknown_role_evidence = locator_evidence("button");
    unknown_role_evidence.role = LocatorField::Unknown;

    let error = match anchor_role_verdict(&entry, &unknown_role_evidence) {
        Err(error) => error,
        Ok(()) => panic!("an unknown role must never verify as a match"),
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
    assert!(error.is_transient_incomplete_read());
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("retryable"))
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "a role read that failed must be stamped explicitly retryable"
    );
}

/// The role guard's other settled branch, for contrast: a role that was
/// read successfully but disagrees with the stored ref settles stale.
#[test]
fn a_known_but_mismatched_role_settles_stale() {
    let entry = locator_entry("button");
    let evidence = locator_evidence("checkbox");

    let error = match anchor_role_verdict(&entry, &evidence) {
        Err(error) => error,
        Ok(()) => panic!("a mismatched role must never verify as a match"),
    };
    assert_eq!(error.code, ErrorCode::StaleRef);
}
