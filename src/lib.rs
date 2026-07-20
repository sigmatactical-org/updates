//! Sigma Updates — Debian package index + OTA catalog (RAUC metadata).

#![forbid(unsafe_code)]

mod api;
mod bundles;
mod catalog;
mod config;
mod dbc;
mod listing;
mod packages;
mod templates;
mod vss;
mod web;

use std::convert::Infallible;
use std::sync::{Arc, OnceLock};

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
    // The theme's `security_headers` borrows the CSP fragment for as long as
    // the returned filter lives, so the origin is resolved once per process.
    static IDENTITY_ORIGIN: OnceLock<String> = OnceLock::new();
    let identity_origin = IDENTITY_ORIGIN.get_or_init(config::identity_public_origin);
    sigma_theme::warp::security_headers(
        sigma_theme::warp::site_routes(web::routes(), api::routes(catalog)),
        identity_origin,
    )
}
