use serde::{Deserialize, Serialize};

use crate::IdentifierKind;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementIdentifier {
    pub kind: IdentifierKind,
    pub value: String,
}

impl ElementIdentifier {
    /// The identifier of the cell at a zero-based row and column index of its
    /// grid, written the way a spreadsheet addresses a cell: column letters,
    /// then the row counted from 1, so row index 6 and column index 1 give
    /// `B7`.
    pub fn grid_cell(row_index: u32, column_index: u32) -> Self {
        Self {
            kind: IdentifierKind::GridCell,
            value: format!(
                "{}{}",
                column_letters(column_index),
                u64::from(row_index) + 1
            ),
        }
    }
}

fn column_letters(column_index: u32) -> String {
    let mut letters = Vec::new();
    let mut rest = u64::from(column_index) + 1;
    while rest > 0 {
        let digit = u8::try_from((rest - 1) % 26).unwrap_or(0);
        letters.push(char::from(b'A' + digit));
        rest = (rest - 1) / 26;
    }
    letters.iter().rev().collect()
}

#[cfg(test)]
#[path = "element_identifier_tests.rs"]
mod tests;
