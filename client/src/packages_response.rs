//! [`PackagesResponse`].

use serde::Deserialize;
use sigma_updates_deb::DebPackage;

/// One page of `GET /v1/packages`.
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
