//! One published Debian package.

use serde::Serialize;
use sigma_updates_deb::{DependencyExpr, PackageRef};

/// Metadata for a `.deb` in the package feed (from its control file, or the
/// filename when the archive cannot be inspected).
#[derive(Debug, Clone, Serialize)]
pub struct DebPackage {
    /// On-disk filename (`{name}_{version}_{arch}.deb`).
    pub filename: String,
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// Target architecture.
    pub architecture: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Service-relative download URL.
    pub download_path: String,
    /// Runtime dependencies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<DependencyExpr>,
    /// Pre-install dependencies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_depends: Vec<DependencyExpr>,
    /// Virtual packages provided.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provides: Vec<PackageRef>,
    /// Short description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
