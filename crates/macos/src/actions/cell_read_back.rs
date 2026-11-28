//! Judging a table cell's value after a committed write.

/// What a cell shows after a committed write, next to the value written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadBack {
    Matches,
    Empty,
    Differs,
    Unknown,
}

/// Judges the cell's value after a commit. `current` is `None` only when the
/// application was read and published no value; a read that failed is
/// `Unknown` and never an empty cell.
pub(crate) fn classify(current: Option<&str>, wanted: &str) -> ReadBack {
    match current {
        Some(held) if held == wanted => ReadBack::Matches,
        Some(held) if !held.is_empty() => ReadBack::Differs,
        _ if wanted.is_empty() => ReadBack::Matches,
        _ => ReadBack::Empty,
    }
}

#[cfg(test)]
#[path = "cell_read_back_tests.rs"]
mod tests;
