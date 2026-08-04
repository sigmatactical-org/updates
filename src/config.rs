//! Runtime configuration for sigma-updates.
//!
//! Required values are declared in the [`sigma_config::service!`] block and
//! checked by [`validate`] at startup.

sigma_config::service! {
    prefix = "UPDATES";
    role = "updates";
    urls {
        /// Public base URL of this service.
        public_base_url = "PUBLIC_BASE_URL" => "http://127.0.0.1:8080/";
        /// Public base URL of the identity BFF.
        identity_public_base_url = "IDENTITY_PUBLIC_URL" => "http://127.0.0.1:3000/";
        /// Public base URL of the contact service for navbar links.
        contact_public_base_url = "CONTACT_PUBLIC_URL" => "http://127.0.0.1:8083/";
        /// Public base URL of the cart service for navbar links.
        cart_public_base_url = "CART_PUBLIC_URL" => "http://127.0.0.1:8084/";
    }
}

/// Public base without trailing slash (bundle URLs, CSP).
#[must_use]
pub fn public_base_url_trimmed() -> String {
    sigma_config::origin_of(&public_base_url())
}

/// Browser origin of the identity BFF for CSP `connect-src` (no trailing slash).
#[must_use]
pub fn identity_public_origin() -> String {
    sigma_config::origin_of(&identity_public_base_url())
}

/// Directory of `.deb` files this service publishes (default `./packages`).
#[must_use]
pub fn packages_dir() -> std::path::PathBuf {
    SERVICE
        .opt_str("PACKAGES_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("packages"))
}

/// Directory of RAUC update bundles, one subdir per channel (default `./bundles`).
#[must_use]
pub fn bundles_dir() -> std::path::PathBuf {
    SERVICE
        .opt_str("BUNDLES_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("bundles"))
}

/// Local cache directory for mirrored `.dbc` schemas (default `./dbc`).
#[must_use]
pub fn dbc_dir() -> std::path::PathBuf {
    SERVICE
        .opt_str("DBC_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("dbc"))
}

/// Local cache directory for mirrored VSS files (default `./vss`).
#[must_use]
pub fn vss_dir() -> std::path::PathBuf {
    SERVICE
        .opt_str("VSS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("vss"))
}

fn env_or(suffix: &str, default: &str) -> String {
    SERVICE
        .opt_str(suffix)
        .unwrap_or_else(|| default.to_string())
}

/// GitHub `owner/repo` holding the canonical `.dbc` schemas.
#[must_use]
pub fn dbc_github_repo() -> String {
    env_or("DBC_GITHUB_REPO", "sigmatactical-org/sigma-racer-wingman")
}

/// Repo subdirectory the schemas are mirrored from.
#[must_use]
pub fn dbc_github_path() -> String {
    env_or("DBC_GITHUB_PATH", "schemas/can")
}

/// Repo subdirectory holding the VSS signal tree.
#[must_use]
pub fn vss_github_path() -> String {
    env_or("VSS_GITHUB_PATH", "schemas/vss")
}

/// Git ref (branch, tag, or SHA) the schemas are mirrored from.
#[must_use]
pub fn dbc_github_ref() -> String {
    env_or("DBC_GITHUB_REF", "main")
}

/// Pause between DBC mirror passes (default 300s).
#[must_use]
pub fn dbc_sync_interval() -> std::time::Duration {
    let secs = SERVICE
        .opt_str("DBC_SYNC_SECS")
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    std::time::Duration::from_secs(secs)
}

/// Optional GitHub token (rate limits / private mirrors).
#[must_use]
pub fn github_token() -> Option<String> {
    SERVICE
        .opt_str("GITHUB_TOKEN")
        .or_else(|| sigma_config::var("GITHUB_TOKEN"))
}

/// Human-readable mirror source for logs.
#[must_use]
pub fn dbc_github_source() -> String {
    format!(
        "{}:{}@{}",
        dbc_github_repo(),
        dbc_github_path(),
        dbc_github_ref()
    )
}
