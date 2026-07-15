//! One mirrored VSS schema file.

use serde::Serialize;

/// Metadata for a VSS file (signal tree or CAN mapping doc) in the catalog.
#[derive(Debug, Clone, Serialize)]
pub struct VssFile {
    /// On-disk filename.
    pub filename: String,
    /// Filename without its extension.
    pub name: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Service-relative download URL.
    pub download_path: String,
}
