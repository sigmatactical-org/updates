//! [`ClientError`].

use thiserror::Error;

/// Why a sigma-updates API call failed.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("http error: {0}")]
    Http(String),
    #[error("unexpected status {status}: {body}")]
    Status { status: u16, body: String },
    #[error("json error: {0}")]
    Json(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}
