use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentifierKind {
    AxIdentifier,
    AxDomIdentifier,
    AutomationId,
    RuntimeId,
    AtspiObjectPath,
    /// The position of a cell in its table or grid, as the platform's table
    /// interface reports it. One grid holds one cell per position.
    GridCell,
    Unknown,
}
