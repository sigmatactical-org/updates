//! Sigma Updates — Debian package index + OTA catalog (RAUC metadata).

#![forbid(unsafe_code)]

mod api;
mod bundles;
mod catalog;
pub mod config;
mod dbc;
mod listing;
mod packages;
mod templates;
mod vss;
mod web;

use std::convert::Infallible;
use std::sync::Arc;

use warp::Filter;
use warp::Reply;

pub use catalog::{Catalog, ChannelRelease};
pub use config::public_base_url_trimmed as public_base_url;
pub use dbc::{DbcCatalog, DbcFile, spawn_github_sync as spawn_dbc_sync};
pub use packages::{DebPackage, PackageCatalog};
pub use vss::{VssCatalog, VssFile};

/// List the published `.deb` packages (startup banner, tests).
#[must_use]
pub fn list_packages() -> Vec<DebPackage> {
    listing::list::<PackageCatalog>()
}

/// HTML site + JSON API + theme static assets, with the shared Sigma
/// security headers and themed error pages.
pub fn routes(
    catalog: Arc<Catalog>,
) -> impl Filter<Extract = (impl Reply,), Error = Infallible> + Clone + Send + 'static {
    sigma_theme::warp::security_headers(
        sigma_theme::warp::site_routes(web::routes(), api::routes(catalog)),
        config::identity_public_origin(),
    )
}
