use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SinceStatus {
    Unchanged,
    Delta,
    UnknownBaseline,
    Expired,
    WindowMismatch,
    ScopeMismatch,
    Incomplete,
}

impl SinceStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Delta => "delta",
            Self::UnknownBaseline => "unknown_baseline",
            Self::Expired => "expired",
            Self::WindowMismatch => "window_mismatch",
            Self::ScopeMismatch => "scope_mismatch",
            Self::Incomplete => "incomplete",
        }
    }
}
