//! [`DebError`].

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DebError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a debian package archive: {0}")]
    Archive(String),
    #[error("control parse error: {0}")]
    Control(String),
}
