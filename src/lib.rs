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
mod web;

use std::convert::Infallible;
use std::sync::Arc;

use warp::Filter;
use warp::Reply;

pub use catalog::{Catalog, ChannelRelease};
pub use config::public_base_url_trimmed as public_base_url;
pub use dbc::{DbcFile, list_dbc_files};
pub use packages::{DebPackage, list_packages};

/// Bind address from `LISTEN_ADDR` (default port 30080).
pub fn listen_addr() -> std::net::SocketAddr {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    std::net::SocketAddr::from(([0, 0, 0, 0], port))
}

fn content_security_policy() -> String {
    let identity_origin = config::identity_public_origin();
    format!(
        "default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'none'; \
         img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; \
         font-src 'self'; connect-src 'self' {identity_origin}; form-action 'self'"
    )
}

/// HTML site + JSON API + theme static assets.
pub fn routes(
    catalog: Arc<Catalog>,
) -> impl Filter<Extract = (impl Reply,), Error = Infallible> + Clone + Send + 'static {
    use warp::reply::with::header;

    web::routes()
        .or(api::routes(catalog))
        .or(sigma_theme::warp::static_files())
        .or(sigma_theme::warp::favicon())
        .recover(sigma_theme::warp::handle_rejection)
        .with(header("content-security-policy", content_security_policy()))
        .with(header("x-content-type-options", "nosniff"))
        .with(header("x-frame-options", "DENY"))
        .with(header("referrer-policy", "strict-origin-when-cross-origin"))
}
