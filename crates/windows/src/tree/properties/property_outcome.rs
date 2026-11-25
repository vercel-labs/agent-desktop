use agent_desktop_core::{LocatorField, Rect};

/// Longest string carried into evidence.
///
/// A value past the bound is `Unknown` rather than a truncated `Known`: a
/// prefix that is presented as exact identity evidence would make
/// re-identification match the wrong element.
pub const MAX_EVIDENCE_CHARS: usize = 2_048;

/// One property read, in the three states core's `LocatorField` distinguishes.
///
/// UI Automation has no per-property error channel. macOS gets a parallel
/// array where an absent slot is `kCFNull` and a failed slot carries its own
/// error; UIA has neither, so this type is built by hand from the
/// not-supported sentinel, the variant tag, and the call's own result.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyOutcome {
    /// The provider answered with a value.
    Known(PropertyValue),
    /// The provider answered, and does not implement this property.
    Absent,
    /// The read failed, or its answer cannot be trusted as identity evidence.
    Unknown,
    /// The provider answered with text longer than `MAX_EVIDENCE_CHARS`; the
    /// count is its length in characters. It is an observation, not a failed
    /// read, and it is never cut into a prefix.
    Oversize(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Text(String),
    Flag(bool),
    Number(i32),
    Real(f64),
    Bounds(Rect),
}

impl PropertyOutcome {
    pub fn text(&self) -> LocatorField<String> {
        match self {
            Self::Known(PropertyValue::Text(value)) => LocatorField::Known(value.clone()),
            Self::Known(_) => LocatorField::Unknown,
            Self::Absent => LocatorField::Absent,
            Self::Unknown | Self::Oversize(_) => LocatorField::Unknown,
        }
    }

    /// The read's text, or `None` if it is missing, unreadable, or blank once
    /// trimmed - the guard every evidence-building call site needs before
    /// trusting a string as identity or descriptive text.
    pub fn non_blank_text(&self) -> Option<String> {
        match self.text() {
            LocatorField::Known(value) if !value.trim().is_empty() => Some(value),
            _ => None,
        }
    }

    /// The character count of an answer that exceeded the evidence bound.
    pub fn oversize_length(&self) -> Option<usize> {
        match self {
            Self::Oversize(length) => Some(*length),
            _ => None,
        }
    }

    pub fn flag(&self) -> Option<bool> {
        match self {
            Self::Known(PropertyValue::Flag(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn number(&self) -> Option<i32> {
        match self {
            Self::Known(PropertyValue::Number(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn real(&self) -> Option<f64> {
        match self {
            Self::Known(PropertyValue::Real(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn bounds(&self) -> LocatorField<Rect> {
        match self {
            Self::Known(PropertyValue::Bounds(value)) => LocatorField::Known(*value),
            Self::Known(_) => LocatorField::Unknown,
            Self::Absent => LocatorField::Absent,
            Self::Unknown | Self::Oversize(_) => LocatorField::Unknown,
        }
    }
}
