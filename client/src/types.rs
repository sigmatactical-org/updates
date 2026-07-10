use serde::{Deserialize, Serialize};
use sigma_updates_deb::{DependencyExpr, PackageRef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebPackage {
    pub filename: String,
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub size_bytes: u64,
    pub download_path: String,
    #[serde(default)]
    pub depends: Vec<DependencyExpr>,
    #[serde(default)]
    pub pre_depends: Vec<DependencyExpr>,
    #[serde(default)]
    pub provides: Vec<PackageRef>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackagesResponse {
    pub packages: Vec<DebPackage>,
    #[serde(default)]
    pub total: usize,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    #[serde(default = "default_page")]
    pub total_pages: u32,
    #[serde(default)]
    pub query: String,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    50
}
