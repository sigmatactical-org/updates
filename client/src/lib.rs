//! HTTP client for the sigma-updates package index.

#![forbid(unsafe_code)]

mod agent;
mod client_error;
mod oidc;
mod packages_response;
mod push;
mod updates_client;

pub use agent::{build_agent, shared_agent};
pub use client_error::ClientError;
pub use oidc::{client_credentials_token, token_url_from_issuer};
pub use packages_response::PackagesResponse;
pub use push::{
    LocalPackage, MissingDependency, PushPlan, PushReport, check_packages, collect_deb_paths,
    load_local_packages, plan_push, push_packages,
};
pub use updates_client::UpdatesClient;

/// The published-package wire type, owned by the `.deb` crate.
pub use sigma_updates_deb::DebPackage;
