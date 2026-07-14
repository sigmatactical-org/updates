//! [`PackageRow`].

#[allow(unused_imports)]
use super::*;

#[derive(Debug)]
pub struct PackageRow {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub size_label: String,
    pub download_path: String,
    pub filename: String,
}
