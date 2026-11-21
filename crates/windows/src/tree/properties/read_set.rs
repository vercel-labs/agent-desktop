//! Which properties one walk reads, chosen by what the walk is for.

use super::property_ids::TreeProperty;

/// The property set a walk requests from the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadSet {
    /// The whole walk set: everything a snapshot publishes.
    Full,
    /// Only what identifies an element: role, name, description, value,
    /// identifiers and the child structure. Geometry, states and actions are
    /// not read, so they stay unknown for the whole walk.
    Identity,
}

/// What identity evidence is built from, and why each property is here.
///
/// - Role: `ControlType`, refined by `ToggleAvailable`, `ExpandCollapseAvailable`,
///   `SelectionAvailable`, `GridItemAvailable`, `TableItemAvailable`,
///   `ValueAvailable` with `ValueIsReadOnly`, `WindowAvailable` and `IsDialog`.
/// - Name and description: `Name`, `FullDescription` and `HelpText` (its
///   fallback), `LabeledBy` (a borrowed name) and `IsPassword`, which withholds
///   any of them from a secure field.
/// - Value, which the stable name can use: `Value`, and `ClassName`, which
///   decides whether a RichEdit paragraph mark is dropped from it.
/// - Identifiers: `AutomationId`.
pub const IDENTITY_SET: [TreeProperty; 18] = [
    TreeProperty::Name,
    TreeProperty::AutomationId,
    TreeProperty::ClassName,
    TreeProperty::HelpText,
    TreeProperty::FullDescription,
    TreeProperty::LabeledBy,
    TreeProperty::Value,
    TreeProperty::IsPassword,
    TreeProperty::IsDialog,
    TreeProperty::ControlType,
    TreeProperty::ValueIsReadOnly,
    TreeProperty::ToggleAvailable,
    TreeProperty::ExpandCollapseAvailable,
    TreeProperty::SelectionAvailable,
    TreeProperty::ValueAvailable,
    TreeProperty::WindowAvailable,
    TreeProperty::GridItemAvailable,
    TreeProperty::TableItemAvailable,
];

impl ReadSet {
    /// The set a walk uses.
    ///
    /// A Chromium root always reads the full set: whether a node is a
    /// transparent wrapper depends on the actions it advertises, so the shape
    /// of its tree depends on properties the identity set leaves out.
    pub fn for_walk(identity_only: bool, chromium_provenance: bool) -> Self {
        if identity_only && !chromium_provenance {
            Self::Identity
        } else {
            Self::Full
        }
    }

    pub fn properties(self) -> &'static [TreeProperty] {
        match self {
            Self::Full => &TreeProperty::WALK_SET,
            Self::Identity => &IDENTITY_SET,
        }
    }

    pub fn reads_geometry_states_and_actions(self) -> bool {
        self == Self::Full
    }
}

#[cfg(test)]
#[path = "read_set_tests.rs"]
mod tests;
