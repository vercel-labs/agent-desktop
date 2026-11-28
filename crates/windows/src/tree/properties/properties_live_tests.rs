use super::*;
use crate::system::test_support::apartment::bootstrap;

/// Pins the fix that keeps a deadline-truncated bounded read from opening the
/// secure-field gate: `read_with` used to `continue` past a truncated
/// property without recording anything, so once the deadline expired before
/// `IsPassword` was reached, its entry was missing from the read set
/// entirely rather than present as `Unknown` - and `ElementProperties::
/// from_reads` treats a missing `IsPassword` entry exactly like a read set
/// that never requested the property at all, defaulting `is_secure()` to
/// `false`. An already-expired deadline forces every property past `Name` to
/// truncate, so this exercises the real production path
/// (`properties::read_live_bounded`) rather than a hand-built read set:
/// `is_secure()` must still come back `true`, and nothing on the element
/// must read back as a genuine, un-withheld answer.
#[test]
fn a_deadline_truncated_bounded_read_still_closes_the_secure_gate() {
    bootstrap();
    let client = crate::tree::automation::automation_client().expect("a UIA client");
    let root: crate::tree::element::UIAElement = client
        .get_root_element()
        .expect("the desktop root element is available")
        .into();

    let expired = agent_desktop_core::Deadline::after(1).expect("a short deadline");
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(
        expired.is_expired(),
        "the deadline must already be expired before the bounded read runs"
    );

    let (properties, errors) = read_live_bounded(&root, expired);

    assert!(
        !errors.is_empty(),
        "a fully truncated read must surface the deadline's own timeout"
    );
    assert!(
        properties.is_secure(),
        "a deadline-truncated IsPassword read must fail closed rather than default the gate open"
    );
    for property in TreeProperty::VALUE_BEARING {
        assert_eq!(
            properties.get(property),
            PropertyOutcome::Unknown,
            "{} must stay unread rather than fabricate an answer",
            property.as_str()
        );
    }
}
