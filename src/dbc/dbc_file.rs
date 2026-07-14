//! One published CAN dictionary.

use serde::Serialize;

/// Metadata for a `.dbc` in the schema catalog.
#[derive(Debug, Clone, Serialize)]
pub struct DbcFile {
    /// On-disk filename.
    pub filename: String,
    /// Filename without the `.dbc` extension.
    pub name: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Service-relative download URL.
    pub download_path: String,
}
