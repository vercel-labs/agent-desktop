use thiserror::Error;

use crate::{AdapterError, ErrorCode};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    /// A permission denial keeps its own code rather than collapsing into
    /// `INTERNAL`. The two are not interchangeable to a caller: `INTERNAL`
    /// says the tool broke and carries no action, while `PERM_DENIED` names a
    /// condition the caller can do something about. Every other io kind stays
    /// `INTERNAL`, because guessing a code from an errno is how an unrelated
    /// failure acquires a confident, wrong label.
    pub fn code(&self) -> &str {
        match self {
            Self::Adapter(error) => error.code.as_str(),
            Self::Io(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                ErrorCode::PermDenied.as_str()
            }
            Self::Io(_) | Self::Json(_) | Self::Internal(_) => "INTERNAL",
        }
    }

    pub fn suggestion(&self) -> Option<&str> {
        self.adapter_error()
            .and_then(|error| error.suggestion.as_deref())
    }

    /// The error as one `AdapterError`, the form every route reports: an
    /// `AdapterError` it carries, directly or inside an io error, whole with
    /// its suggestion and details; otherwise a permission denial as
    /// `PERM_DENIED` and anything else as `INTERNAL`, with the error's message.
    pub fn into_adapter_error(self) -> AdapterError {
        let message = self.to_string();
        match self {
            Self::Adapter(error) => error,
            Self::Io(error) => match AdapterError::from_io(&error) {
                Some(carried) => carried.clone(),
                None if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    AdapterError::new(ErrorCode::PermDenied, message)
                }
                None => AdapterError::new(ErrorCode::Internal, message),
            },
            Self::Json(_) | Self::Internal(_) => AdapterError::new(ErrorCode::Internal, message),
        }
    }

    pub(crate) fn adapter_error(&self) -> Option<&AdapterError> {
        match self {
            Self::Adapter(error) => Some(error),
            Self::Io(error) => AdapterError::from_io(error),
            _ => None,
        }
    }

    pub fn stale_ref(ref_id: &str) -> Self {
        Self::Adapter(AdapterError::stale_ref(ref_id))
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::Adapter(
            AdapterError::new(ErrorCode::InvalidArgs, message)
                .with_disposition(crate::DeliverySemantics::not_delivered()),
        )
    }

    pub fn invalid_input_with_suggestion(
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self::Adapter(
            AdapterError::new(ErrorCode::InvalidArgs, message)
                .with_suggestion(suggestion)
                .with_disposition(crate::DeliverySemantics::not_delivered()),
        )
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
