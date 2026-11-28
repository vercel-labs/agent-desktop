use super::{IDENTITY_SET, ReadSet};
use crate::tree::properties::property_ids::TreeProperty;

#[test]
fn every_identity_property_is_in_the_walk_set() {
    for property in IDENTITY_SET {
        assert!(
            TreeProperty::WALK_SET.contains(&property),
            "{} is read by the identity walk but not by the full walk",
            property.as_str()
        );
    }
}

#[test]
fn the_gate_of_every_identity_property_is_read_with_it() {
    for property in IDENTITY_SET {
        if let Some(gate) = property.gate() {
            assert!(
                IDENTITY_SET.contains(&gate),
                "{} is read without its gate {}",
                property.as_str(),
                gate.as_str()
            );
        }
    }
}

#[test]
fn the_identity_set_leaves_geometry_states_and_actions_unread() {
    for property in [
        TreeProperty::BoundingRectangle,
        TreeProperty::IsOffscreen,
        TreeProperty::LegacyDefaultAction,
        TreeProperty::LegacyState,
        TreeProperty::LegacyValue,
        TreeProperty::InvokeAvailable,
        TreeProperty::SelectionItemIsSelected,
    ] {
        assert!(!IDENTITY_SET.contains(&property), "{}", property.as_str());
    }
}

#[test]
fn only_a_non_chromium_identity_walk_reads_the_identity_set() {
    assert_eq!(ReadSet::for_walk(true, false), ReadSet::Identity);
    assert_eq!(ReadSet::for_walk(true, true), ReadSet::Full);
    assert_eq!(ReadSet::for_walk(false, false), ReadSet::Full);
    assert_eq!(
        ReadSet::Full.properties().len(),
        TreeProperty::WALK_SET.len()
    );
    assert_eq!(ReadSet::Identity.properties().len(), IDENTITY_SET.len());
}
