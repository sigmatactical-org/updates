//! Failure modes shared by catalog publish/delete operations.

use thiserror::Error;

/// Why a publish or delete was rejected.
#[derive(Debug, Error)]
pub enum PublishError {
    /// Filename empty, wrong extension, or contains path separators.
    #[error("invalid filename")]
    InvalidFilename,
    /// Upload body was empty.
    #[error("empty body")]
    EmptyBody,
    /// Upload exceeds the per-catalog size cap.
    #[error("file too large")]
    TooLarge,
    /// No such published file.
    #[error("file not found")]
    NotFound,
    /// Content failed catalog-specific validation.
    #[error("invalid content: {0}")]
    InvalidContent(String),
    /// Underlying filesystem failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
