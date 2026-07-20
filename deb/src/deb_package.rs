//! [`DebPackage`] — the published-package wire type.

use serde::{Deserialize, Serialize};

use crate::control::DebControl;
use crate::depends::{DependencyExpr, PackageRef};

/// Metadata for a `.deb` in the package feed, as served by the JSON API and
/// consumed by the client (from the archive's control file, or the filename
/// when the archive cannot be inspected).
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl DebPackage {
    /// Build an entry from a parsed control file.
    #[must_use]
    pub fn from_control(
        filename: &str,
        size_bytes: u64,
        download_path: String,
        control: &DebControl,
    ) -> Self {
        Self {
            filename: filename.to_owned(),
            name: control.package.clone(),
            version: control.version.clone(),
            architecture: control.architecture.clone(),
            size_bytes,
            download_path,
            depends: control.depends.clone(),
            pre_depends: control.pre_depends.clone(),
            provides: control.provides.clone(),
            description: control.description.clone(),
        }
    }
}
