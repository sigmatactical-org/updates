//! [`PackageListQuery`].

#[allow(unused_imports)]
use super::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct PackageListQuery {
    pub(crate) page: Option<u32>,
    pub(crate) per_page: Option<u32>,
    pub(crate) q: Option<String>,
}
