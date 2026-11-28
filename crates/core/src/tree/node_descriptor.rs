use serde::{Deserialize, Serialize};

/// The optional presentation descriptors this schema adds: a finer-grained
/// role refinement, the provider's own control-type description, an input
/// placeholder, the DOM class list of a web-rendered element, and the size of
/// a table or grid.
///
/// Every field is absent by default, so a node that supplies none of them
/// serializes byte-identically to a node that predates this schema (the macOS
/// golden fixtures are the regression proof). The group flattened inside
/// [`crate::NodePresentation`] so it appears at the same level a top-level
/// placement would, without growing that struct past its field cap.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dom_classes: Vec<String>,
    /// How many rows a table or grid reports through the platform's table
    /// interface. A spreadsheet can report far more than it shows.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub row_count: Option<u32>,
    /// How many columns that table or grid reports.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub column_count: Option<u32>,
}
